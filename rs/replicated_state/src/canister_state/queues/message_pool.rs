// TODO(MR-569) Remove when `CanisterQueues` has been updated to use this.
#![allow(dead_code)]

use ic_protobuf::proxy::{try_from_option_field, ProxyDecodeError};
use ic_protobuf::state::queues::v1 as pb_queues;
use ic_types::messages::{
    Request, RequestOrResponse, Response, MAX_RESPONSE_COUNT_BYTES, NO_DEADLINE,
};
use ic_types::time::CoarseTime;
use ic_types::{CountBytes, Time};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::ops::{AddAssign, SubAssign};
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
pub(super) mod tests;

/// The lifetime of a guaranteed response call request in an output queue, from
/// which its deadline is computed (as `now + REQUEST_LIFETIME`).
pub const REQUEST_LIFETIME: Duration = Duration::from_secs(300);

/// Bit encoding the message kind (request or response).
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Request = 0,
    Response = Self::BIT,
}

impl Kind {
    // Message kind bit (request or response).
    const BIT: u64 = 1;
}

impl From<&RequestOrResponse> for Kind {
    fn from(msg: &RequestOrResponse) -> Self {
        match msg {
            RequestOrResponse::Request(_) => Kind::Request,
            RequestOrResponse::Response(_) => Kind::Response,
        }
    }
}

/// Bit encoding the message context (inbound or outbound).
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    Inbound = 0,
    Outbound = Self::BIT,
}

impl Context {
    // Message context bit (inbound or outbound).
    const BIT: u64 = 1 << 1;
}

/// Bit encoding the message class (guaranteed response vs best-effort).
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Class {
    GuaranteedResponse = 0,
    BestEffort = Self::BIT,
}

impl Class {
    // Message class bit (guaranteed response vs best-effort).
    const BIT: u64 = 1 << 2;
}

impl From<&RequestOrResponse> for Class {
    fn from(msg: &RequestOrResponse) -> Self {
        match msg {
            RequestOrResponse::Request(req) if req.deadline == NO_DEADLINE => {
                Class::GuaranteedResponse
            }
            RequestOrResponse::Response(rep) if rep.deadline == NO_DEADLINE => {
                Class::GuaranteedResponse
            }
            RequestOrResponse::Request(_) | RequestOrResponse::Response(_) => Class::BestEffort,
        }
    }
}

/// A unique generated identifier for a message held in a `MessagePool` that
/// also encodes the message kind (request or response) and context (incoming or
/// outgoing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessageId(u64);

impl MessageId {
    /// Number of `MessageId` bits used as flags.
    const BITMASK_LEN: u32 = 3;

    fn new(kind: Kind, context: Context, class: Class, generator: u64) -> Self {
        Self(kind as u64 | context as u64 | class as u64 | generator << MessageId::BITMASK_LEN)
    }

    pub(super) fn kind(&self) -> Kind {
        if self.0 & Kind::BIT == Kind::Request as u64 {
            Kind::Request
        } else {
            Kind::Response
        }
    }

    pub(super) fn context(&self) -> Context {
        if self.0 & Context::BIT == Context::Inbound as u64 {
            Context::Inbound
        } else {
            Context::Outbound
        }
    }

    pub(super) fn class(&self) -> Class {
        if self.0 & Class::BIT == Class::GuaranteedResponse as u64 {
            Class::GuaranteedResponse
        } else {
            Class::BestEffort
        }
    }
}

/// A placeholder for a potential late inbound best-effort response.
///
/// Does not implement `Clone` or `Copy` to ensure that it can only be used
/// once.
pub(super) struct ResponsePlaceholder(MessageId);

impl ResponsePlaceholder {
    /// Returns the message ID within.
    pub(super) fn id(&self) -> MessageId {
        self.0
    }
}

/// A pool of canister messages, guaranteed response and best effort, with
/// built-in support for time-based expiration and load shedding.
///
/// Messages in the pool are identified by a `MessageId` generated by the pool.
/// The `MessageId` also encodes the message kind (request or response); and
/// context (inbound or outbound).
///
/// Messages are added to the deadline queue based on their class (best-effort
/// vs guaranteed response) and context: i.e. all best-effort messages except
/// responses in input queues; plus guaranteed response call requests in output
/// queues. All best-effort messages (and only best-effort messages) are added
/// to the load shedding queue.
///
/// All pool operations except `expire_messages()` and
/// `calculate_message_stats()` (only called during deserialization) execute in
/// at most `O(log(N))` time.
#[derive(Clone, Debug, Default)]
pub struct MessagePool {
    /// Pool contents.
    messages: BTreeMap<MessageId, RequestOrResponse>,

    /// Running message stats for the pool.
    message_stats: MessageStats,

    /// Deadline priority queue, earliest deadlines first.
    ///
    /// Message IDs break ties, ensuring deterministic representation across
    /// replicas.
    deadline_queue: BinaryHeap<(Reverse<CoarseTime>, MessageId)>,

    /// Load shedding priority queue: largest message first.
    ///
    /// Message IDs break ties, ensuring deterministic representation across
    /// replicas.
    size_queue: BinaryHeap<(usize, MessageId)>,

    /// A monotonically increasing counter used to generate unique message IDs.
    message_id_generator: u64,
}

impl MessagePool {
    /// Inserts an inbound message (one that is to be enqueued in an input queue)
    /// into the pool. Returns the ID assigned to the message.
    ///
    /// The message is added to the deadline queue iff it is a best-effort request
    /// (best effort responses that already made it into an input queue should not
    /// expire). It is added to the load shedding queue if it is a best-effort
    /// message.
    pub(crate) fn insert_inbound(&mut self, msg: RequestOrResponse) -> MessageId {
        let deadline = match &msg {
            RequestOrResponse::Request(request) => request.deadline,

            // Never expire responses already enqueued in an input queue.
            RequestOrResponse::Response(_) => NO_DEADLINE,
        };

        self.insert_impl(msg, deadline, Context::Inbound)
    }

    /// Inserts an outbound request (one that is to be enqueued in an output queue)
    /// into the pool. Returns the ID assigned to the request.
    ///
    /// The request is always added to the deadline queue: if it is a best-effort
    /// request, with its explicit deadline; if it is a guaranteed response call
    /// request, with a deadline of `now + REQUEST_LIFETIME`. It is added to the
    /// load shedding queue iff it is a best-effort request.
    pub(crate) fn insert_outbound_request(
        &mut self,
        request: Arc<Request>,
        now: Time,
    ) -> MessageId {
        let deadline = if request.deadline == NO_DEADLINE {
            // Guaranteed response call requests in canister output queues expire after
            // `REQUEST_LIFETIME`.
            CoarseTime::floor(now + REQUEST_LIFETIME)
        } else {
            // Best-effort requests expire as per their specified deadline.
            request.deadline
        };

        self.insert_impl(
            RequestOrResponse::Request(request),
            deadline,
            Context::Outbound,
        )
    }

    /// Inserts an outbound response (one that is to be enqueued in an output queue)
    /// into the pool. Returns the ID assigned to the response.
    ///
    /// The response is added to both the deadline queue and the load shedding queue
    /// iff it is a best-effort response.
    pub(crate) fn insert_outbound_response(&mut self, response: Arc<Response>) -> MessageId {
        let deadline = response.deadline;
        self.insert_impl(
            RequestOrResponse::Response(response),
            deadline,
            Context::Outbound,
        )
    }

    /// Inserts the given message into the pool with the provided `deadline` (rather
    /// than the message's actual deadline; this is so we can expire the outgoing
    /// requests of guaranteed response calls; and not expire incoming best-effort
    /// responses). Returns the ID assigned to the message.
    ///
    /// The message is recorded into the deadline queue with the provided `deadline`
    /// iff that is non-zero. It is recorded in the load shedding priority queue iff
    /// the message is a best-effort message.
    fn insert_impl(
        &mut self,
        msg: RequestOrResponse,
        deadline: CoarseTime,
        context: Context,
    ) -> MessageId {
        let id = self.next_message_id(Kind::from(&msg), context, Class::from(&msg));

        let size_bytes = msg.count_bytes();
        let is_best_effort = msg.is_best_effort();

        // Update message stats.
        self.message_stats += MessageStats::stats_delta(&msg, context);

        // Insert.
        assert!(self.messages.insert(id, msg).is_none());
        debug_assert_eq!(
            Self::calculate_message_stats(&self.messages),
            self.message_stats
        );

        // Record in deadline queue iff a deadline was provided.
        if deadline != NO_DEADLINE {
            self.deadline_queue.push((Reverse(deadline), id));
        }

        // Record in load shedding queue iff it's a best-effort message.
        if is_best_effort {
            self.size_queue.push((size_bytes, id));
        }

        id
    }

    /// Prepares a placeholder for a potential late inbound best-effort response.
    pub(super) fn insert_inbound_timeout_response(&mut self) -> ResponsePlaceholder {
        ResponsePlaceholder(self.next_message_id(
            Kind::Response,
            Context::Inbound,
            Class::BestEffort,
        ))
    }

    /// Inserts a late inbound best-effort response into a response placeholder.
    pub(super) fn replace_inbound_timeout_response(
        &mut self,
        placeholder: ResponsePlaceholder,
        msg: RequestOrResponse,
    ) {
        // Message must be a best-effort response.
        match &msg {
            RequestOrResponse::Response(rep) if rep.deadline != NO_DEADLINE => {}
            _ => panic!("Message must be a best-effort response"),
        }

        let id = placeholder.0;
        debug_assert!(Context::Inbound == id.context());
        debug_assert!(Class::BestEffort == id.class());
        debug_assert!(Kind::Response == id.kind());
        let size_bytes = msg.count_bytes();

        // Update message stats.
        self.message_stats += MessageStats::stats_delta(&msg, id.context());

        // Insert. Cannot lead to a conflict because the placeholder is consumed on use.
        assert!(self.messages.insert(id, msg).is_none());
        debug_assert_eq!(
            Self::calculate_message_stats(&self.messages),
            self.message_stats
        );

        // Record in load shedding queue only.
        self.size_queue.push((size_bytes, id));
    }

    /// Reserves and returns a new message ID.
    fn next_message_id(&mut self, kind: Kind, context: Context, class: Class) -> MessageId {
        let id = MessageId::new(kind, context, class, self.message_id_generator);
        self.message_id_generator += 1;
        id
    }

    /// Retrieves the request with the given `MessageId`.
    ///
    /// Panics if the provided ID was generated for a `Response`.
    pub(crate) fn get_request(&self, id: MessageId) -> Option<&RequestOrResponse> {
        assert_eq!(Kind::Request, id.kind());

        self.messages.get(&id)
    }

    /// Retrieves the response with the given `MessageId`.
    ///
    /// Panics if the provided ID was generated for a `Request`.
    pub(crate) fn get_response(&self, id: MessageId) -> Option<&RequestOrResponse> {
        assert_eq!(Kind::Response, id.kind());

        self.messages.get(&id)
    }

    /// Retrieves the message with the given `MessageId`.
    pub(crate) fn get(&self, id: MessageId) -> Option<&RequestOrResponse> {
        self.messages.get(&id)
    }

    /// Removes the message with the given `MessageId` from the pool.
    ///
    /// Updates the stats; and prunes the priority queues if necessary.
    pub(crate) fn take(&mut self, id: MessageId) -> Option<RequestOrResponse> {
        let msg = self.messages.remove(&id)?;

        self.message_stats -= MessageStats::stats_delta(&msg, id.context());
        debug_assert_eq!(
            Self::calculate_message_stats(&self.messages),
            self.message_stats
        );

        self.maybe_trim_queues();

        Some(msg)
    }

    /// Queries whether the deadline at the head of the deadline queue has expired.
    ///
    /// This is a fast check, but it may produce false positives if the message at
    /// the head of the deadline queue has already been removed from the pool.
    ///
    /// Time complexity: `O(1)`.
    pub(crate) fn has_expired_deadlines(&self, now: Time) -> bool {
        if let Some((deadline, _)) = self.deadline_queue.peek() {
            let now = CoarseTime::floor(now);
            if deadline.0 < now {
                return true;
            }
        }
        false
    }

    /// Removes and returns all messages with expired deadlines (i.e. `deadline <
    /// now`).
    ///
    /// Amortized time complexity per expired message: `O(log(n))`.
    pub(crate) fn expire_messages(&mut self, now: Time) -> Vec<(MessageId, RequestOrResponse)> {
        if self.deadline_queue.is_empty() {
            return Vec::new();
        }

        let now = CoarseTime::floor(now);
        let mut expired = Vec::new();
        while let Some((deadline, id)) = self.deadline_queue.peek() {
            if deadline.0 >= now {
                break;
            }
            let id = *id;

            // Pop the deadline queue entry.
            self.deadline_queue.pop();

            // Drop the message, if present.
            if let Some(msg) = self.take(id) {
                expired.push((id, msg))
            }
        }

        expired
    }

    /// Removes and returns the largest best-effort message in the pool.
    pub(crate) fn shed_largest_message(&mut self) -> Option<(MessageId, RequestOrResponse)> {
        // Keep trying until we actually drop a message.
        while let Some((_, id)) = self.size_queue.pop() {
            if let Some(msg) = self.take(id) {
                return Some((id, msg));
            }
        }

        // Nothing to shed.
        None
    }

    /// Returns the number of messages in the pool.
    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }

    /// Returns a reference to the pool's message stats.
    pub(super) fn message_stats(&self) -> &MessageStats {
        &self.message_stats
    }

    /// Prunes stale entries from the priority queues if they make up more than half
    /// of the respective priority queue. This ensures amortized constant time for
    /// the priority queues.
    fn maybe_trim_queues(&mut self) {
        let len = self.messages.len();

        if len == 0 {
            *self = MessagePool::default();
            return;
        }

        if self.deadline_queue.len() > 2 * len + 2 {
            self.deadline_queue
                .retain(|&(_, id)| self.messages.contains_key(&id));
        }
        if self.size_queue.len() > 2 * len + 2 {
            self.size_queue
                .retain(|&(_, id)| self.messages.contains_key(&id));
        }
    }

    /// Computes message stats from scratch. Used when deserializing and in
    /// `debug_assert!()` checks.
    ///
    /// Time complexity: `O(n)`.
    fn calculate_message_stats(messages: &BTreeMap<MessageId, RequestOrResponse>) -> MessageStats {
        let mut stats = MessageStats::default();
        for (id, msg) in messages.iter() {
            stats += MessageStats::stats_delta(msg, id.context());
        }
        stats
    }

    fn check_invariants(&self) -> Result<(), String> {
        const OUTBOUND_GUARANTEED_REQUEST: u64 =
            Context::Outbound as u64 | Class::GuaranteedResponse as u64 | Kind::Request as u64;
        const MESSAGE_ID_FLAGS_MASK: u64 = Context::BIT | Class::BIT | Kind::BIT;

        // To compare the largest seen `MessageId` against `message_id_generator`.
        let mut max_message_id = 0;

        // Collect all message IDs that should be present in the deadline / size queues.
        let mut best_effort_messages = BTreeSet::new();
        let mut messages_with_deadlines = BTreeSet::new();

        self.messages.iter().try_for_each(|(id, msg)| {
            // `MessageId` kind and class must match those of the message.
            if id.kind() != Kind::from(msg) {
                return Err(format!(
                    "Message kind mismatch: message {:?}, MessageId {:?}",
                    Kind::from(msg),
                    id.kind()
                ));
            }
            if id.class() != Class::from(msg) {
                return Err(format!(
                    "Message class mismatch: message {:?}, MessageId {:?}",
                    Class::from(msg),
                    id.class()
                ));
            }

            // Collect all the relevant `MessageIds`.
            max_message_id = max_message_id.max(id.0);
            if id.class() == Class::BestEffort {
                best_effort_messages.insert(id);
                messages_with_deadlines.insert(id);
            } else if id.0 & MESSAGE_ID_FLAGS_MASK == OUTBOUND_GUARANTEED_REQUEST {
                messages_with_deadlines.insert(id);
            }

            Ok(())
        })?;

        // Validate `message_id_generator`.
        if max_message_id >> MessageId::BITMASK_LEN >= self.message_id_generator {
            return Err(format!(
                "MessageId out of bounds: max MessageId: {}, message_id_generator: {}",
                max_message_id, self.message_id_generator
            ));
        }

        self.deadline_queue.iter().try_for_each(|(deadline, id)| {
            if id.class() == Class::GuaranteedResponse && id.0 & MESSAGE_ID_FLAGS_MASK != OUTBOUND_GUARANTEED_REQUEST {
                return Err(format!(
                    "Unexpected MessageId in deadline queue: MessageId {:?}, kind: {:?}, context: {:?}, class: {:?}",
                    id, id.kind(), id.context(), id.class()
                ));
            }
            // Ensure that all best-effort messages' deadlines match what's in `deadline_queue`.
            if id.class() == Class::BestEffort && messages_with_deadlines.contains(id) {
                let msg = self.messages.get(id).unwrap();
                if msg.deadline() != deadline.0 {
                    return Err(format!(
                        "Deadline mismatch: MessageId {:?}, message: {:?}, deadline_queue: {:?}",
                        id,
                        msg.deadline(),
                        deadline.0
                    ));
                }
            }
            messages_with_deadlines.remove(id);
            Ok(())
        })?;
        // All best-effort messages and outbound guaranteed response requests must be
        // present in the deadline queue.
        if !messages_with_deadlines.is_empty() {
            return Err(format!(
                "Messages missing from deadline queue: {:?}",
                messages_with_deadlines
            ));
        }

        self.size_queue.iter().try_for_each(|(_, id)| {
            if id.class() == Class::GuaranteedResponse {
                return Err(format!(
                    "Guaranteed response message in load shedding queue: MessageId {:?}, kind: {:?}, context: {:?}, class: {:?}",
                    id, id.kind(), id.context(), id.class()
                ));
            }
            best_effort_messages.remove(id);
            Ok(())
        })?;
        // All best-effort messages must be present in the load shedding queue.
        if !best_effort_messages.is_empty() {
            return Err(format!(
                "Best-effort messages missing from load shedding queue: {:?}",
                best_effort_messages
            ));
        }

        Ok(())
    }
}

impl PartialEq for MessagePool {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            messages,
            message_stats,
            deadline_queue,
            size_queue,
            message_id_generator: next_message_id_generator,
        } = self;

        if *messages != other.messages
            || *message_stats != other.message_stats
            || *next_message_id_generator != other.message_id_generator
        {
            return false;
        }

        return binary_heap_eq(deadline_queue, &other.deadline_queue)
            && binary_heap_eq(size_queue, &other.size_queue);
    }
}
impl Eq for MessagePool {}

/// Compares two binary heaps for equality of their contents, allowing for
/// different representations.
///
/// This is a rather expensive check, if the representations are different, but
/// it's only used during checkpointing.
fn binary_heap_eq<T: Eq + Ord + Clone>(lhs: &BinaryHeap<T>, rhs: &BinaryHeap<T>) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }

    // First do a cheap equality comparison, in case the two heaps do have identical
    // representations.
    if lhs
        .iter()
        .zip(rhs.iter())
        .all(|(lhs_entry, rhs_entry)| lhs_entry == rhs_entry)
    {
        // Happy path.
        return true;
    }

    // Slow path: sort the heaps and compare the sorted vectors.
    lhs.clone()
        .into_sorted_vec()
        .iter()
        .zip(rhs.clone().into_sorted_vec().iter())
        .all(|(lhs_entry, rhs_entry)| lhs_entry == rhs_entry)
}

impl From<&MessagePool> for pb_queues::MessagePool {
    fn from(item: &MessagePool) -> Self {
        use pb_queues::message_pool::*;

        Self {
            messages: item
                .messages
                .iter()
                .map(|(message_id, message)| Entry {
                    message_id: message_id.0,
                    message: Some(message.into()),
                })
                .collect(),
            message_deadlines: item
                .deadline_queue
                .clone()
                .into_sorted_vec()
                .iter()
                .map(|(deadline, message_id)| MessageDeadline {
                    deadline_seconds: deadline.0.as_secs_since_unix_epoch(),
                    message_id: message_id.0,
                })
                .collect(),
            message_sizes: item
                .size_queue
                .clone()
                .into_sorted_vec()
                .iter()
                .map(|(size_bytes, message_id)| MessageSize {
                    size_bytes: *size_bytes as u64,
                    message_id: message_id.0,
                })
                .collect(),
            message_id_generator: item.message_id_generator,
        }
    }
}

impl TryFrom<pb_queues::MessagePool> for MessagePool {
    type Error = ProxyDecodeError;
    fn try_from(item: pb_queues::MessagePool) -> Result<Self, Self::Error> {
        let message_count = item.messages.len();

        let messages: BTreeMap<_, _> = item
            .messages
            .into_iter()
            .map(|entry| {
                let message_id = MessageId(entry.message_id);
                let message = try_from_option_field(entry.message, "MessagePool::Entry::message")?;
                Ok((message_id, message))
            })
            .collect::<Result<_, Self::Error>>()?;
        if messages.len() != message_count {
            return Err(ProxyDecodeError::Other(format!("Duplicate MessageId")));
        }

        let message_stats = Self::calculate_message_stats(&messages);

        let deadline_queue = item
            .message_deadlines
            .into_iter()
            .map(|entry| {
                let deadline = CoarseTime::from_secs_since_unix_epoch(entry.deadline_seconds);
                let message_id = MessageId(entry.message_id);
                (Reverse(deadline), message_id)
            })
            .collect();
        let size_queue = item
            .message_sizes
            .into_iter()
            .map(|entry| {
                let size_bytes = entry.size_bytes as usize;
                let message_id = MessageId(entry.message_id);
                (size_bytes, message_id)
            })
            .collect();

        let res = Self {
            messages,
            message_stats,
            deadline_queue,
            size_queue,
            message_id_generator: item.message_id_generator,
        };
        res.check_invariants().map_err(ProxyDecodeError::Other)?;

        Ok(res)
    }
}

/// Running stats for all messages in a `MessagePool`.
///
/// Slot reservations and memory reservations for guaranteed responses, being
/// queue metrics, are tracked separately by `CanisterQueues`.
///
/// All operations (computing stats deltas and retrieving the stats) are
/// constant time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct MessageStats {
    /// Total byte size of all messages in the pool.
    pub(super) size_bytes: usize,

    /// Total byte size of all best-effort messages in the pool.
    pub(super) best_effort_message_bytes: usize,

    /// Total byte size of all guaranteed responses in the pool.
    pub(super) guaranteed_responses_size_bytes: usize,

    /// Sum total of bytes above `MAX_RESPONSE_COUNT_BYTES` per oversized guaranteed
    /// response call request. Execution allows local-subnet requests larger than
    /// `MAX_RESPONSE_COUNT_BYTES`.
    pub(super) oversized_guaranteed_requests_extra_bytes: usize,

    /// Total byte size of all messages in input queue.
    pub(super) inbound_size_bytes: usize,

    /// Count of messages in input queues.
    pub(super) inbound_message_count: usize,

    /// Count of responses in input queues.
    pub(super) inbound_response_count: usize,

    /// Count of guaranteed response requests in input queues.
    ///
    /// At the end of each round, this plus the number of not yet responded
    /// guaranteed response call contexts must be equal to the number of guaranteed
    /// response memory reservations for inbound calls.
    pub(super) inbound_guaranteed_request_count: usize,

    /// Count of guaranteed responses in input queues.
    ///
    /// At the end of each round, the number of guaranteed response callbacks minus
    /// this must be equal to the number of guaranteed response memory reservations
    /// for outbound calls.
    pub(super) inbound_guaranteed_response_count: usize,

    /// Count of messages in output queues.
    pub(super) outbound_message_count: usize,
}

impl MessageStats {
    /// Returns the memory usage of the guaranteed response messages in the pool,
    /// excluding memory reservations for guaranteed responses.
    ///
    /// Complexity: `O(1)`.
    pub fn guaranteed_response_memory_usage(&self) -> usize {
        self.guaranteed_responses_size_bytes + self.oversized_guaranteed_requests_extra_bytes
    }

    /// Calculates the change in stats caused by pushing (+) or popping (-) the
    /// given message in the given context.
    fn stats_delta(msg: &RequestOrResponse, context: Context) -> MessageStats {
        match msg {
            RequestOrResponse::Request(req) => Self::request_stats_delta(req, context),
            RequestOrResponse::Response(rep) => Self::response_stats_delta(rep, context),
        }
    }

    /// Calculates the change in stats caused by pushing (+) or popping (-) the
    /// given request in the given context.
    fn request_stats_delta(req: &Request, context: Context) -> MessageStats {
        use Class::*;
        use Context::*;

        let size_bytes = req.count_bytes();
        let class = if req.deadline == NO_DEADLINE {
            GuaranteedResponse
        } else {
            BestEffort
        };

        // This is a request, response stats are all unaffected.
        let guaranteed_responses_size_bytes = 0;
        let inbound_response_count = 0;
        let inbound_guaranteed_response_count = 0;

        match (context, class) {
            (Inbound, GuaranteedResponse) => MessageStats {
                size_bytes,
                best_effort_message_bytes: 0,
                guaranteed_responses_size_bytes,
                oversized_guaranteed_requests_extra_bytes: size_bytes
                    .saturating_sub(MAX_RESPONSE_COUNT_BYTES),
                inbound_size_bytes: size_bytes,
                inbound_message_count: 1,
                inbound_response_count,
                inbound_guaranteed_request_count: 1,
                inbound_guaranteed_response_count,
                outbound_message_count: 0,
            },
            (Inbound, BestEffort) => MessageStats {
                size_bytes,
                best_effort_message_bytes: size_bytes,
                guaranteed_responses_size_bytes,
                oversized_guaranteed_requests_extra_bytes: 0,
                inbound_size_bytes: size_bytes,
                inbound_message_count: 1,
                inbound_response_count,
                inbound_guaranteed_request_count: 0,
                inbound_guaranteed_response_count,
                outbound_message_count: 0,
            },
            (Outbound, GuaranteedResponse) => MessageStats {
                size_bytes,
                best_effort_message_bytes: 0,
                guaranteed_responses_size_bytes,
                oversized_guaranteed_requests_extra_bytes: size_bytes
                    .saturating_sub(MAX_RESPONSE_COUNT_BYTES),
                inbound_size_bytes: 0,
                inbound_message_count: 0,
                inbound_response_count,
                inbound_guaranteed_request_count: 0,
                inbound_guaranteed_response_count,
                outbound_message_count: 1,
            },
            (Outbound, BestEffort) => MessageStats {
                size_bytes,
                best_effort_message_bytes: size_bytes,
                guaranteed_responses_size_bytes,
                oversized_guaranteed_requests_extra_bytes: 0,
                inbound_size_bytes: 0,
                inbound_message_count: 0,
                inbound_response_count,
                inbound_guaranteed_request_count: 0,
                inbound_guaranteed_response_count,
                outbound_message_count: 1,
            },
        }
    }

    /// Calculates the change in stats caused by pushing (+) or popping (-) the
    /// given response in the given context.
    fn response_stats_delta(rep: &Response, context: Context) -> MessageStats {
        use Class::*;
        use Context::*;

        let size_bytes = rep.count_bytes();
        let class = if rep.deadline == NO_DEADLINE {
            GuaranteedResponse
        } else {
            BestEffort
        };

        // This is a response, request stats are all unaffected.
        let oversized_guaranteed_requests_extra_bytes = 0;
        let inbound_guaranteed_request_count = 0;

        match (context, class) {
            (Inbound, GuaranteedResponse) => MessageStats {
                size_bytes,
                best_effort_message_bytes: 0,
                guaranteed_responses_size_bytes: size_bytes,
                oversized_guaranteed_requests_extra_bytes,
                inbound_size_bytes: size_bytes,
                inbound_message_count: 1,
                inbound_response_count: 1,
                inbound_guaranteed_request_count,
                inbound_guaranteed_response_count: 1,
                outbound_message_count: 0,
            },
            (Inbound, BestEffort) => MessageStats {
                size_bytes,
                best_effort_message_bytes: size_bytes,
                guaranteed_responses_size_bytes: 0,
                oversized_guaranteed_requests_extra_bytes,
                inbound_size_bytes: size_bytes,
                inbound_message_count: 1,
                inbound_response_count: 1,
                inbound_guaranteed_request_count,
                inbound_guaranteed_response_count: 0,
                outbound_message_count: 0,
            },
            (Outbound, GuaranteedResponse) => MessageStats {
                size_bytes,
                best_effort_message_bytes: 0,
                guaranteed_responses_size_bytes: size_bytes,
                oversized_guaranteed_requests_extra_bytes,
                inbound_size_bytes: 0,
                inbound_message_count: 0,
                inbound_response_count: 0,
                inbound_guaranteed_request_count,
                inbound_guaranteed_response_count: 0,
                outbound_message_count: 1,
            },
            (Outbound, BestEffort) => MessageStats {
                size_bytes,
                best_effort_message_bytes: size_bytes,
                guaranteed_responses_size_bytes: 0,
                oversized_guaranteed_requests_extra_bytes,
                inbound_size_bytes: 0,
                inbound_message_count: 0,
                inbound_response_count: 0,
                inbound_guaranteed_request_count,
                inbound_guaranteed_response_count: 0,
                outbound_message_count: 1,
            },
        }
    }
}

impl AddAssign<MessageStats> for MessageStats {
    fn add_assign(&mut self, rhs: MessageStats) {
        let MessageStats {
            size_bytes,
            best_effort_message_bytes,
            guaranteed_responses_size_bytes,
            oversized_guaranteed_requests_extra_bytes,
            inbound_size_bytes,
            inbound_message_count,
            inbound_response_count,
            inbound_guaranteed_request_count,
            inbound_guaranteed_response_count,
            outbound_message_count,
        } = rhs;
        self.size_bytes += size_bytes;
        self.best_effort_message_bytes += best_effort_message_bytes;
        self.guaranteed_responses_size_bytes += guaranteed_responses_size_bytes;
        self.oversized_guaranteed_requests_extra_bytes += oversized_guaranteed_requests_extra_bytes;
        self.inbound_size_bytes += inbound_size_bytes;
        self.inbound_message_count += inbound_message_count;
        self.inbound_response_count += inbound_response_count;
        self.inbound_guaranteed_request_count += inbound_guaranteed_request_count;
        self.inbound_guaranteed_response_count += inbound_guaranteed_response_count;
        self.outbound_message_count += outbound_message_count;
    }
}

impl SubAssign<MessageStats> for MessageStats {
    fn sub_assign(&mut self, rhs: MessageStats) {
        let MessageStats {
            size_bytes,
            best_effort_message_bytes,
            guaranteed_responses_size_bytes,
            oversized_guaranteed_requests_extra_bytes,
            inbound_size_bytes,
            inbound_message_count,
            inbound_response_count,
            inbound_guaranteed_request_count,
            inbound_guaranteed_response_count,
            outbound_message_count,
        } = rhs;
        self.size_bytes -= size_bytes;
        self.best_effort_message_bytes -= best_effort_message_bytes;
        self.guaranteed_responses_size_bytes -= guaranteed_responses_size_bytes;
        self.oversized_guaranteed_requests_extra_bytes -= oversized_guaranteed_requests_extra_bytes;
        self.inbound_size_bytes -= inbound_size_bytes;
        self.inbound_message_count -= inbound_message_count;
        self.inbound_response_count -= inbound_response_count;
        self.inbound_guaranteed_request_count -= inbound_guaranteed_request_count;
        self.inbound_guaranteed_response_count -= inbound_guaranteed_response_count;
        self.outbound_message_count -= outbound_message_count;
    }
}
