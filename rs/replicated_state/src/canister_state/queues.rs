mod message_pool;
mod queue;
#[cfg(test)]
mod tests;

use self::message_pool::{Context, Kind, MessagePool};
use self::queue::{CanisterQueue, CanisterQueueItem, IngressQueue};
use crate::replicated_state::MR_SYNTHETIC_REJECT_MESSAGE_MAX_LEN;
use crate::{CanisterState, CheckpointLoadingMetrics, InputQueueType, NextInputQueue, StateError};
use ic_base_types::PrincipalId;
use ic_error_types::RejectCode;
use ic_management_canister_types::IC_00;
use ic_protobuf::proxy::{try_from_option_field, ProxyDecodeError};
use ic_protobuf::state::queues::v1 as pb_queues;
use ic_protobuf::state::queues::v1::canister_queues::{
    CanisterQueuePair, NextInputQueue as ProtoNextInputQueue,
};
use ic_protobuf::types::v1 as pb_types;
use ic_types::messages::{
    CallbackId, CanisterMessage, Ingress, Payload, RejectContext, Request, RequestOrResponse,
    Response, MAX_RESPONSE_COUNT_BYTES, NO_DEADLINE,
};
use ic_types::{CanisterId, CountBytes, Time};
use ic_validate_eq::ValidateEq;
use ic_validate_eq_derive::ValidateEq;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::convert::{From, TryFrom};
use std::sync::Arc;

pub const DEFAULT_QUEUE_CAPACITY: usize = 500;

/// Encapsulates information about `CanisterQueues`,
/// used in detecting a loop when consuming the input messages.
#[derive(Clone, Debug, Default, PartialEq, Eq, ValidateEq)]
pub struct CanisterQueuesLoopDetector {
    pub local_queue_skip_count: usize,
    pub remote_queue_skip_count: usize,
    pub ingress_queue_skip_count: usize,
}

impl CanisterQueuesLoopDetector {
    /// Detects a loop in `CanisterQueues`.
    pub fn detected_loop(&self, canister_queues: &CanisterQueues) -> bool {
        let skipped_all_remote =
            self.remote_queue_skip_count >= canister_queues.remote_subnet_input_schedule.len();

        let skipped_all_local =
            self.local_queue_skip_count >= canister_queues.local_subnet_input_schedule.len();

        let skipped_all_ingress =
            self.ingress_queue_skip_count >= canister_queues.ingress_queue.ingress_schedule_size();

        // An empty queue is skipped implicitly by `peek_input()` and `pop_input()`.
        // This means that no new messages can be consumed from an input source if
        // - either it is empty,
        // - or all its queues were explicitly skipped.
        // Note that `skipped_all_remote`, `skipped_all_local`, and `skipped_all_ingress`
        // are trivially true if the corresponding input source is empty because empty
        // queues are removed from the source.
        skipped_all_remote && skipped_all_local && skipped_all_ingress
    }
}

/// Wrapper around the induction pool (ingress and input queues); a priority
/// queue used for round-robin scheduling of senders when consuming input
/// messages; and output queues.
///
/// Responsible for queue lifetime management, fair scheduling of inputs across
/// sender canisters and queue backpressure.
///
/// Encapsulates the `InductionPool` component described in the spec. The reason
/// for bundling together the induction pool and output queues is to reliably
/// implement backpressure via queue slot reservations for response messages.
#[derive(Clone, Debug, Default, PartialEq, Eq, ValidateEq)]
pub struct CanisterQueues {
    /// Queue of ingress (user) messages.
    #[validate_eq(CompareWithValidateEq)]
    ingress_queue: IngressQueue,

    /// Per remote canister input and output queues. Queues hold references into the
    /// message pool, some of which may be stale due to expiration or load shedding.
    /// The item at the head of each queue, if any, is guaranteed to be non-stale.
    #[validate_eq(CompareWithValidateEq)]
    canister_queues: BTreeMap<CanisterId, (CanisterQueue, CanisterQueue)>,

    /// Pool holding the messages referenced by `canister_queues`, with support for
    /// time-based expiration and load shedding.
    #[validate_eq(Ignore)]
    pool: MessagePool,

    /// Slot and memory reservation stats. Message count and size stats are
    /// maintained separately in the `MessagePool`.
    queue_stats: QueueStats,

    /// FIFO queue of local subnet sender canister IDs ensuring round-robin
    /// consumption of input messages. All local senders with non-empty queues
    /// are scheduled.
    ///
    /// We rely on `ReplicatedState::canister_states` to decide whether a canister
    /// is local or not. This test is subject to race conditions (e.g. if the sender
    /// has just been deleted), meaning that the separation into local and remote
    /// senders is best effort.
    local_subnet_input_schedule: VecDeque<CanisterId>,

    /// FIFO queue of remote subnet sender canister IDs ensuring round-robin
    /// consumption of input messages. All remote senders with non-empty queues
    /// are scheduled.
    ///
    /// We rely on `ReplicatedState::canister_states` to decide whether a canister
    /// is local or not. This test is subject to race conditions (e.g. if the sender
    /// has just been deleted), meaning that the separation into local and remote
    /// senders is best effort.
    remote_subnet_input_schedule: VecDeque<CanisterId>,

    /// Set of all canisters enqueued in either `local_subnet_input_schedule` or
    /// `remote_subnet_input_schedule`, to ensure that a canister is scheduled at
    /// most once.
    input_schedule_canisters: BTreeSet<CanisterId>,

    /// Round-robin across ingress and cross-net input queues for `pop_input()`.
    #[validate_eq(Ignore)]
    next_input_queue: NextInputQueue,

    /// The callback IDs of all responses enqueued in the input queues.
    ///
    /// Used for response deduplication (whether due to a locally generated reject
    /// response to a best-effort call; or due to a malicious / buggy subnet).
    callbacks_with_enqueued_response: BTreeSet<CallbackId>,
}

/// Circular iterator that consumes output queue messages: loops over output
/// queues, popping one message at a time from each in a round robin fashion.
/// All messages that have not been explicitly popped will remain in the state.
///
/// Additional operations compared to a standard iterator:
///  * peeking (returning a reference to the next message without consuming it);
///    and
///  * excluding whole queues from iteration while retaining their messages
///    (e.g. in order to efficiently implement per destination limits).
#[derive(Debug)]
pub struct CanisterOutputQueuesIterator<'a> {
    /// Priority queue of non-empty output queues. The next message to be popped
    /// / peeked is the one at the head of the first queue.
    queues: VecDeque<(&'a CanisterId, &'a mut CanisterQueue)>,

    pool: &'a mut MessagePool,

    /// Number of (potentially stale) messages left in the iterator.
    size: usize,
}

impl<'a> CanisterOutputQueuesIterator<'a> {
    /// Creates a new output queue iterator from the given
    /// `CanisterQueues::canister_queues` (a map of `CanisterId` to an input queue,
    /// output queue pair) and `MessagePool`.
    fn new(
        queues: &'a mut BTreeMap<CanisterId, (CanisterQueue, CanisterQueue)>,
        pool: &'a mut MessagePool,
    ) -> Self {
        let queues: VecDeque<_> = queues
            .iter_mut()
            .filter(|(_, (_, queue))| queue.len() > 0)
            .map(|(canister, (_, queue))| (canister, queue))
            .collect();
        let size = Self::compute_size(&queues);

        CanisterOutputQueuesIterator { queues, pool, size }
    }

    /// Returns the first message from the next queue.
    pub fn peek(&self) -> Option<&RequestOrResponse> {
        let item = self.queues.front()?.1.peek().unwrap();

        let msg = self.pool.get(item.id());
        assert!(msg.is_some(), "stale reference at the head of output queue");
        msg
    }

    /// Pops the first message from the next queue.
    ///
    /// Advances the queue to the next non-stale message. If such a message exists,
    /// the queue is moved to the back of the iteration order, else it is dropped.
    pub fn pop(&mut self) -> Option<RequestOrResponse> {
        let (receiver, queue) = self.queues.pop_front()?;
        debug_assert!(self.size >= queue.len());
        self.size -= queue.len();

        // Queue must be non-empty and message at the head of queue non-stale.
        let msg = pop_and_advance(queue, self.pool).unwrap();
        debug_assert_eq!(Ok(()), canister_queue_ok(queue, self.pool, receiver));

        if queue.len() > 0 {
            self.size += queue.len();
            self.queues.push_back((receiver, queue));
        }
        debug_assert_eq!(Self::compute_size(&self.queues), self.size);

        Some(msg)
    }

    /// Permanently excludes from iteration the next queue (i.e. all messages
    /// with the same sender and receiver as the next message). The messages are
    /// retained in the output queue.
    ///
    /// Returns the number of (potentially stale) messages left in the just excluded
    /// queue.
    pub fn exclude_queue(&mut self) -> usize {
        let ignored = self
            .queues
            .pop_front()
            .map(|(_, q)| q.len())
            .unwrap_or_default();

        debug_assert!(self.size >= ignored);
        self.size -= ignored;
        debug_assert_eq!(Self::compute_size(&self.queues), self.size);

        ignored
    }

    /// Checks if the iterator has finished.
    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }

    /// Returns the number of (potentially stale) messages left in the iterator.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Computes the number of (potentially stale) messages left in `queues`.
    ///
    /// Time complexity: O(N).
    fn compute_size(queues: &VecDeque<(&'a CanisterId, &'a mut CanisterQueue)>) -> usize {
        queues.iter().map(|(_, q)| q.len()).sum()
    }
}

impl Iterator for CanisterOutputQueuesIterator<'_> {
    type Item = RequestOrResponse;

    /// Alias for `pop`.
    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }

    /// Returns the bounds on the number of messages remaining in the iterator.
    ///
    /// Since any message reference may or may not be stale (due to expiration /
    /// load shedding), there may be anywhere between 0 and `size` messages left in
    /// the iterator.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.size))
    }
}

impl CanisterQueues {
    /// Pushes an ingress message into the induction pool.
    pub fn push_ingress(&mut self, msg: Ingress) {
        self.ingress_queue.push(msg)
    }

    /// Pops the next ingress message from `ingress_queue`.
    fn pop_ingress(&mut self) -> Option<Arc<Ingress>> {
        self.ingress_queue.pop()
    }

    /// Peeks the next ingress message from `ingress_queue`.
    fn peek_ingress(&self) -> Option<Arc<Ingress>> {
        self.ingress_queue.peek()
    }

    /// For each output queue, invokes `f` on every message until `f` returns
    /// `Err`; then moves on to the next output queue.
    ///
    /// All messages that `f` returned `Ok` for, are popped. Messages that `f`
    /// returned `Err` for and all those following them in the respective output
    /// queue are retained.
    ///
    /// Do note that because a queue can only be skipped over if `f` returns `Err`
    /// on a non-stale message, queues are always either fully consumed or left with
    /// a non-stale reference at the front.
    pub(crate) fn output_queues_for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(&CanisterId, &RequestOrResponse) -> Result<(), ()>,
    {
        for (canister_id, (_, queue)) in self.canister_queues.iter_mut() {
            while let Some(item) = queue.peek() {
                let id = item.id();
                let Some(msg) = self.pool.get(id) else {
                    // Expired / dropped message. Pop it and advance.
                    assert_eq!(Some(*item), queue.pop());
                    continue;
                };

                match f(canister_id, msg) {
                    // `f` rejected the message, move on to next queue.
                    Err(_) => break,

                    // Message consumed, pop it.
                    Ok(_) => {
                        self.pool
                            .take(id)
                            .expect("ger() returned a message, take() should not fail");
                        assert_eq!(Some(*item), queue.pop());
                    }
                }
            }
        }

        debug_assert_eq!(Ok(()), self.test_invariants());
    }

    /// Returns an iterator that loops over output queues, popping one message
    /// at a time from each in a round robin fashion. The iterator consumes all
    /// popped messages.
    pub(crate) fn output_into_iter(&mut self) -> CanisterOutputQueuesIterator {
        CanisterOutputQueuesIterator::new(&mut self.canister_queues, &mut self.pool)
    }

    /// See `IngressQueue::filter_messages()` for documentation.
    pub fn filter_ingress_messages<F>(&mut self, filter: F) -> Vec<Arc<Ingress>>
    where
        F: FnMut(&Arc<Ingress>) -> bool,
    {
        self.ingress_queue.filter_messages(filter)
    }

    /// Pushes a canister-to-canister message into the induction pool.
    ///
    /// If the message is a `Request` this will also reserve a slot in the
    /// corresponding output queue for the eventual response.
    ///
    /// If the message is a `Response` the protocol will have already reserved a
    /// slot for it, so the push should not fail due to the input queue being full
    /// (although an error will be returned in case of a bug in the upper layers).
    ///
    /// Adds the sender to the appropriate input schedule (local or remote), if not
    /// already there.
    ///
    /// # Errors
    ///
    /// If pushing fails, returns the provided message along with a
    /// `StateError`:
    ///
    ///  * `QueueFull` if pushing a `Request` and the corresponding input or
    ///    output queues are full.
    ///
    ///  * `NonMatchingResponse` if pushing a `Response` and the corresponding input
    ///    queue does not have a reserved slot; or this is a duplicate guaranteed
    ///    response.
    pub(super) fn push_input(
        &mut self,
        msg: RequestOrResponse,
        input_queue_type: InputQueueType,
    ) -> Result<(), (StateError, RequestOrResponse)> {
        fn non_matching_response(message: &str, response: &Response) -> StateError {
            StateError::NonMatchingResponse {
                err_str: message.to_string(),
                originator: response.originator,
                callback_id: response.originator_reply_callback,
                respondent: response.respondent,
                deadline: response.deadline,
            }
        }

        let sender = msg.sender();
        let input_queue = match &msg {
            RequestOrResponse::Request(_) => {
                let (input_queue, output_queue) = self.get_or_insert_queues(&sender);
                if let Err(e) = input_queue.check_has_request_slot() {
                    return Err((e, msg));
                }
                // Safe to already (attempt to) reserve an output slot here, as the `push()`
                // below is guaranteed to succeed due to the check above.
                if let Err(e) = output_queue.try_reserve_response_slot() {
                    return Err((e, msg));
                }
                // Make the borrow checker happy.
                &mut self.canister_queues.get_mut(&sender).unwrap().0
            }
            RequestOrResponse::Response(response) => {
                match self.canister_queues.get_mut(&sender) {
                    Some((queue, _)) if queue.check_has_reserved_response_slot().is_ok() => {
                        // Check against duplicate responses.
                        if !self
                            .callbacks_with_enqueued_response
                            .insert(response.originator_reply_callback)
                        {
                            // This is a critical error for guaranteed responses.
                            if response.deadline == NO_DEADLINE {
                                return Err((
                                    non_matching_response("Duplicate response", response),
                                    msg,
                                ));
                            } else {
                                // But is OK for best-effort responses (if we already generated a timeout response).
                                // Silently ignore the response.
                                return Ok(());
                            }
                        }
                        queue
                    }

                    // Queue does not exist or has no reserved slot for this response.
                    _ => {
                        return Err((
                            non_matching_response("No reserved response slot", response),
                            msg,
                        ));
                    }
                }
            }
        };

        self.queue_stats.on_push(&msg, Context::Inbound);
        let id = self.pool.insert_inbound(msg);
        match id.kind() {
            Kind::Request => input_queue.push_request(id),
            Kind::Response => input_queue.push_response(id),
        }

        // Add sender canister ID to the appropriate input schedule queue if it is not
        // already scheduled.
        if input_queue.len() == 1 && self.input_schedule_canisters.insert(sender) {
            match input_queue_type {
                InputQueueType::LocalSubnet => self.local_subnet_input_schedule.push_back(sender),
                InputQueueType::RemoteSubnet => self.remote_subnet_input_schedule.push_back(sender),
            }
        }

        debug_assert_eq!(Ok(()), self.test_invariants());
        Ok(())
    }

    /// Pops the next canister input queue message.
    ///
    /// Note: We pop senders from the head of `input_schedule` and insert them
    /// to the back, which allows us to handle messages from different
    /// originators in a round-robin fashion.
    fn pop_canister_input(&mut self, input_queue: InputQueueType) -> Option<CanisterMessage> {
        let input_schedule = match input_queue {
            InputQueueType::LocalSubnet => &mut self.local_subnet_input_schedule,
            InputQueueType::RemoteSubnet => &mut self.remote_subnet_input_schedule,
        };

        // It is possible for an input schedule to contain an empty or garbage collected
        // input queue if all messages in said queue have expired / were shed since it
        // was scheduled. Meaning that iteration may be required.
        while let Some(sender) = input_schedule.pop_front() {
            let Some((input_queue, _)) = self.canister_queues.get_mut(&sender) else {
                // Queue pair was garbage collected.
                assert!(self.input_schedule_canisters.remove(&sender));
                continue;
            };
            let msg = pop_and_advance(input_queue, &mut self.pool);

            // If the input queue is non-empty, re-enqueue the sender at the back of the
            // input schedule queue.
            if input_queue.len() != 0 {
                input_schedule.push_back(sender);
            } else {
                assert!(self.input_schedule_canisters.remove(&sender));
            }

            if let Some(msg) = msg {
                if let RequestOrResponse::Response(response) = &msg {
                    assert!(self
                        .callbacks_with_enqueued_response
                        .remove(&response.originator_reply_callback));
                }
                debug_assert_eq!(Ok(()), self.test_invariants());
                return Some(msg.into());
            }
        }

        debug_assert_eq!(Ok(()), self.test_invariants());
        None
    }

    /// Peeks the next canister input queue message.
    ///
    /// It is possible for an input schedule to contain an empty input queue if e.g.
    /// all messages in said queue have expired / were shed since it was scheduled.
    /// Meaning that it may be necessary to iterate over and mutate input schedules
    /// in order to achieve amortized `O(1)` time complexity.
    fn peek_canister_input(&mut self, input_queue: InputQueueType) -> Option<CanisterMessage> {
        let input_schedule = match input_queue {
            InputQueueType::LocalSubnet => &mut self.local_subnet_input_schedule,
            InputQueueType::RemoteSubnet => &mut self.remote_subnet_input_schedule,
        };

        while let Some(sender) = input_schedule.front() {
            // The sender's input queue.
            let Some((input_queue, _)) = self.canister_queues.get_mut(sender) else {
                // Queue pair was garbage collected.
                assert!(self.input_schedule_canisters.remove(sender));
                input_schedule.pop_front();
                continue;
            };

            if let Some(item) = input_queue.peek() {
                let msg = self
                    .pool
                    .get(item.id())
                    .expect("stale reference at the head of input queue");
                debug_assert_eq!(Ok(()), self.test_invariants());
                return Some(msg.clone().into());
            }

            // All messages in the queue had expired / were shed.
            input_schedule.pop_front();
        }

        debug_assert_eq!(Ok(()), self.test_invariants());
        None
    }

    /// Skips the next canister input queue message.
    fn skip_canister_input(&mut self, input_queue: InputQueueType) {
        let input_schedule = match input_queue {
            InputQueueType::LocalSubnet => &mut self.local_subnet_input_schedule,
            InputQueueType::RemoteSubnet => &mut self.remote_subnet_input_schedule,
        };
        if let Some(sender) = input_schedule.pop_front() {
            let input_queue = &mut self.canister_queues.get_mut(&sender).unwrap().0;
            if input_queue.len() != 0 {
                input_schedule.push_back(sender);
            } else {
                assert!(self.input_schedule_canisters.remove(&sender));
            }
        }
        debug_assert_eq!(Ok(()), self.test_invariants());
    }

    /// Returns `true` if `ingress_queue` or at least one of the canister input
    /// queues contains non-stale messages; `false` otherwise.
    pub fn has_input(&self) -> bool {
        !self.ingress_queue.is_empty() || self.pool.message_stats().inbound_message_count > 0
    }

    /// Returns `true` if at least one output queue contains non-stale messages;
    /// false otherwise.
    pub fn has_output(&self) -> bool {
        self.pool.message_stats().outbound_message_count > 0
    }

    /// Peeks the ingress or inter-canister input message that would be returned by
    /// `pop_input()`.
    ///
    /// Requires a `&mut self` reference in order to be able to drop empty queues
    /// from the input schedule, in order to achieve amortized `O(1)` time complexity.
    pub(crate) fn peek_input(&mut self) -> Option<CanisterMessage> {
        // Try all 3 inputs: Ingress, Local, and Remote subnets
        for _ in 0..3 {
            let next_input = match self.next_input_queue {
                NextInputQueue::Ingress => self.peek_ingress().map(CanisterMessage::Ingress),
                NextInputQueue::RemoteSubnet => {
                    self.peek_canister_input(InputQueueType::RemoteSubnet)
                }
                NextInputQueue::LocalSubnet => {
                    self.peek_canister_input(InputQueueType::LocalSubnet)
                }
            };

            match next_input {
                Some(msg) => return Some(msg),
                // Try another input queue.
                None => {
                    self.next_input_queue = match self.next_input_queue {
                        NextInputQueue::LocalSubnet => NextInputQueue::Ingress,
                        NextInputQueue::Ingress => NextInputQueue::RemoteSubnet,
                        NextInputQueue::RemoteSubnet => NextInputQueue::LocalSubnet,
                    }
                }
            }
        }

        None
    }

    /// Skips the next ingress or inter-canister input message.
    pub(crate) fn skip_input(&mut self, loop_detector: &mut CanisterQueuesLoopDetector) {
        let current_input_queue = self.next_input_queue;
        match current_input_queue {
            NextInputQueue::Ingress => {
                self.ingress_queue.skip_ingress_input();
                loop_detector.ingress_queue_skip_count += 1;
                self.next_input_queue = NextInputQueue::RemoteSubnet
            }

            NextInputQueue::RemoteSubnet => {
                self.skip_canister_input(InputQueueType::RemoteSubnet);
                loop_detector.remote_queue_skip_count += 1;
                self.next_input_queue = NextInputQueue::LocalSubnet;
            }

            NextInputQueue::LocalSubnet => {
                self.skip_canister_input(InputQueueType::LocalSubnet);
                loop_detector.local_queue_skip_count += 1;
                self.next_input_queue = NextInputQueue::Ingress;
            }
        }
    }

    /// Pops the next ingress or inter-canister input message (round-robin).
    ///
    /// We define three buckets of queues: messages from canisters on the same
    /// subnet (local subnet), ingress, and messages from canisters on other
    /// subnets (remote subnet).
    ///
    /// Each time this function is called, we round robin between these three
    /// buckets. We also round robin between the queues in the local subnet and
    /// remote subnet buckets when we pop messages from those buckets.
    pub(crate) fn pop_input(&mut self) -> Option<CanisterMessage> {
        // Try all 3 inputs: Ingress, Local, and Remote subnets
        for _ in 0..3 {
            let cur_input_queue = self.next_input_queue;
            // Switch to the next input queue
            self.next_input_queue = match self.next_input_queue {
                NextInputQueue::LocalSubnet => NextInputQueue::Ingress,
                NextInputQueue::Ingress => NextInputQueue::RemoteSubnet,
                NextInputQueue::RemoteSubnet => NextInputQueue::LocalSubnet,
            };

            let next_input = match cur_input_queue {
                NextInputQueue::Ingress => self.pop_ingress().map(CanisterMessage::Ingress),

                NextInputQueue::RemoteSubnet => {
                    self.pop_canister_input(InputQueueType::RemoteSubnet)
                }

                NextInputQueue::LocalSubnet => self.pop_canister_input(InputQueueType::LocalSubnet),
            };

            if next_input.is_some() {
                return next_input;
            }
        }

        None
    }

    /// Pushes a `Request` into the relevant output queue. Also reserves a slot for
    /// the eventual response in the matching input queue.
    ///
    /// # Errors
    ///
    /// Returns a `QueueFull` error along with the provided message if either
    /// the output queue or the matching input queue is full.
    pub fn push_output_request(
        &mut self,
        request: Arc<Request>,
        time: Time,
    ) -> Result<(), (StateError, Arc<Request>)> {
        let (input_queue, output_queue) = self.get_or_insert_queues(&request.receiver);

        if let Err(e) = output_queue.check_has_request_slot() {
            return Err((e, request));
        }
        if let Err(e) = input_queue.try_reserve_response_slot() {
            return Err((e, request));
        }
        // Make the borrow checker happy.
        let (_, output_queue) = &mut self.canister_queues.get_mut(&request.receiver).unwrap();

        self.queue_stats
            .on_push_request(&request, Context::Outbound);

        let id = self.pool.insert_outbound_request(request, time);
        output_queue.push_request(id);

        debug_assert_eq!(Ok(()), self.test_invariants());
        Ok(())
    }

    /// Immediately reject an output request by pushing a `Response` onto the
    /// input queue without ever putting the `Request` on an output queue. This
    /// can only be used for `IC00` requests and requests to subnet IDs.
    ///
    /// This is expected to be used in cases of invalid sender canister version
    /// in management canister calls and `IC00` routing where no
    /// destination subnet is found that the `Request` could be routed to
    /// or if the canister directly includes subnet IDs in the request.
    /// Hence, an immediate (reject) `Response` is added to the relevant
    /// input queue.
    pub(crate) fn reject_subnet_output_request(
        &mut self,
        request: Request,
        reject_context: RejectContext,
        subnet_ids: &[PrincipalId],
    ) -> Result<(), StateError> {
        assert!(
            request.receiver == IC_00 || subnet_ids.contains(&request.receiver.get()),
            "reject_subnet_output_request can only be used to reject management canister requests"
        );

        let (input_queue, _output_queue) = self.get_or_insert_queues(&request.receiver);
        input_queue.try_reserve_response_slot()?;
        self.queue_stats
            .on_push_request(&request, Context::Outbound);
        debug_assert_eq!(Ok(()), self.test_invariants());

        let response = RequestOrResponse::Response(Arc::new(Response {
            originator: request.sender,
            respondent: request.receiver,
            originator_reply_callback: request.sender_reply_callback,
            refund: request.payment,
            response_payload: Payload::Reject(reject_context),
            deadline: request.deadline,
        }));
        self.push_input(response, InputQueueType::LocalSubnet)
            .map_err(|(e, _msg)| e)
    }

    /// Returns the number of output requests that can be pushed to each
    /// canister before either the respective input or output queue is full.
    pub fn available_output_request_slots(&self) -> BTreeMap<CanisterId, usize> {
        // When pushing a request we need to reserve a slot on the input
        // queue for the eventual reply. So we are limited by the amount of
        // space in both the output and input queues.
        self.canister_queues
            .iter()
            .map(|(canister, (input_queue, output_queue))| {
                (
                    *canister,
                    input_queue
                        .available_response_slots()
                        .min(output_queue.available_request_slots()),
                )
            })
            .collect()
    }

    /// Pushes a `Response` into the relevant output queue. The protocol should have
    /// already reserved a slot, so this cannot fail.
    ///
    /// # Panics
    ///
    /// Panics if the queue does not already exist or there is no reserved slot
    /// to push the `Response` into.
    pub fn push_output_response(&mut self, response: Arc<Response>) {
        self.queue_stats
            .on_push_response(&response, Context::Outbound);

        // Since we reserve an output queue slot whenever we induct a request; and
        // we would never garbage collect a non-empty queue (including one with just a
        // reserved slot); we are guaranteed that the output queue exists.
        let output_queue = &mut self
            .canister_queues
            .get_mut(&response.originator)
            .expect("pushing response into inexistent output queue")
            .1;
        let id = self.pool.insert_outbound_response(response);
        output_queue.push_response(id);

        debug_assert_eq!(Ok(()), self.test_invariants());
    }

    /// Returns a reference to the (non-stale) message at the head of the respective
    /// output queue, if any.
    pub(super) fn peek_output(&self, canister_id: &CanisterId) -> Option<&RequestOrResponse> {
        let output_queue = &self.canister_queues.get(canister_id)?.1;

        let msg = self.pool.get(output_queue.peek()?.id());
        assert!(msg.is_some(), "stale reference at the head of output queue");
        msg
    }

    /// Tries to induct a message from the output queue to `own_canister_id`
    /// into the input queue from `own_canister_id`. Returns `Err(())` if there
    /// was no message to induct or the input queue was full.
    pub(super) fn induct_message_to_self(&mut self, own_canister_id: CanisterId) -> Result<(), ()> {
        let msg = self.peek_output(&own_canister_id).ok_or(())?.clone();

        self.push_input(msg, InputQueueType::LocalSubnet)
            .map_err(|_| ())?;

        let queue = &mut self
            .canister_queues
            .get_mut(&own_canister_id)
            .expect("Output queue existed above so lookup should not fail.")
            .1;
        pop_and_advance(queue, &mut self.pool)
            .expect("Message peeked above so pop should not fail.");

        debug_assert_eq!(Ok(()), self.test_invariants());
        Ok(())
    }

    /// Returns the number of enqueued ingress messages.
    pub fn ingress_queue_message_count(&self) -> usize {
        self.ingress_queue.size()
    }

    /// Returns the total byte size of enqueued ingress messages.
    pub fn ingress_queue_size_bytes(&self) -> usize {
        self.ingress_queue.count_bytes()
    }

    /// Returns the number of non-stale canister messages enqueued in input queues.
    pub fn input_queues_message_count(&self) -> usize {
        self.pool.message_stats().inbound_message_count
    }

    /// Returns the number of reserved slots across all input queues.
    ///
    /// Note that this is different from memory reservations for guaranteed
    /// responses.
    pub fn input_queues_reserved_slots(&self) -> usize {
        self.queue_stats.input_queues_reserved_slots
    }

    /// Returns the total byte size of canister input queues (queues +
    /// messages).
    pub fn input_queues_size_bytes(&self) -> usize {
        self.pool.message_stats().inbound_size_bytes
            + self.canister_queues.len() * size_of::<CanisterQueue>()
    }

    /// Returns the number of non-stale requests enqueued in input queues.
    pub fn input_queues_request_count(&self) -> usize {
        self.pool.message_stats().inbound_message_count
            - self.pool.message_stats().inbound_response_count
    }

    /// Returns the number of non-stale responses enqueued in input queues.
    pub fn input_queues_response_count(&self) -> usize {
        self.pool.message_stats().inbound_response_count
    }

    /// Returns the number of actual (non-stale) messages in output queues.
    pub fn output_queues_message_count(&self) -> usize {
        self.pool.message_stats().outbound_message_count
    }

    /// Returns the number of reserved slots across all output queues.
    ///
    /// Note that this is different from memory reservations for guaranteed
    /// responses.
    pub fn output_queues_reserved_slots(&self) -> usize {
        self.queue_stats.output_queues_reserved_slots
    }

    /// Returns the memory usage of all best-effort messages.
    pub fn best_effort_memory_usage(&self) -> usize {
        self.pool.message_stats().best_effort_message_bytes
    }

    /// Returns the memory usage of all guaranteed response messages.
    pub fn guaranteed_response_memory_usage(&self) -> usize {
        self.queue_stats.guaranteed_response_memory_usage()
            + self.pool.message_stats().guaranteed_response_memory_usage()
    }

    /// Returns the total byte size of guaranteed responses across input and
    /// output queues.
    pub fn guaranteed_responses_size_bytes(&self) -> usize {
        self.pool.message_stats().guaranteed_responses_size_bytes
    }

    /// Returns the total memory reservations for guaranteed responses across input
    /// and output queues.
    ///
    /// Note that this is different from slots reserved for responses (whether
    /// best effort or guaranteed) which are used to implement backpressure.
    pub fn guaranteed_response_memory_reservations(&self) -> usize {
        self.queue_stats.guaranteed_response_memory_reservations
    }

    /// Returns the sum total of bytes above `MAX_RESPONSE_COUNT_BYTES` per
    /// oversized guaranteed response call request.
    pub fn oversized_guaranteed_requests_extra_bytes(&self) -> usize {
        self.pool
            .message_stats()
            .oversized_guaranteed_requests_extra_bytes
    }

    /// Sets the (transient) size in bytes of guaranteed responses routed from
    /// `output_queues` into streams and not yet garbage collected.
    pub(super) fn set_stream_guaranteed_responses_size_bytes(&mut self, size_bytes: usize) {
        self.queue_stats
            .transient_stream_guaranteed_responses_size_bytes = size_bytes;
    }

    /// Returns an existing matching pair of input and output queues from/to
    /// the given canister; or creates a pair of empty queues, if non-existent.
    fn get_or_insert_queues(
        &mut self,
        canister_id: &CanisterId,
    ) -> (&mut CanisterQueue, &mut CanisterQueue) {
        let (input_queue, output_queue) =
            self.canister_queues.entry(*canister_id).or_insert_with(|| {
                let input_queue = CanisterQueue::new(DEFAULT_QUEUE_CAPACITY);
                let output_queue = CanisterQueue::new(DEFAULT_QUEUE_CAPACITY);
                (input_queue, output_queue)
            });
        (input_queue, output_queue)
    }

    /// Garbage collects all input and output queue pairs that are both empty.
    ///
    /// Because there is no useful information in an empty queue, there is no
    /// need to retain them. In order to avoid state divergence (e.g. because
    /// some replicas have an empty queue pair and some have garbage collected
    /// it) we simply need to ensure that queues are garbage collected
    /// deterministically across all replicas (e.g. at checkpointing time or
    /// every round; but not e.g. when deserializing, which may happen at
    /// different times on restarting or state syncing replicas).
    ///
    /// Time complexity: `O(num_queues)`.
    pub fn garbage_collect(&mut self) {
        self.garbage_collect_impl();

        // Reset all fields to default if we have no messages. This is so that an empty
        // `CanisterQueues` serializes as an empty byte array (and there is no need to
        // persist it explicitly).
        if self.canister_queues.is_empty() && self.ingress_queue.is_empty() {
            // The schedules and stats will already have default (zero) values, only
            // `next_input_queue` and `pool` must be reset explicitly.
            self.next_input_queue = Default::default();
            assert!(self.pool.len() == 0);
            self.pool = MessagePool::default();

            // Trust but verify. Ensure everything is actually set to default.
            debug_assert_eq!(CanisterQueues::default(), *self);
        }
    }

    /// Implementation of `garbage_collect()`, ensuring the latter always resets
    /// all fields to their default values when all queues are empty, regardless
    /// of whether we bail out early or not.
    fn garbage_collect_impl(&mut self) {
        if self.canister_queues.is_empty() {
            return;
        }

        self.canister_queues
            .retain(|_canister_id, (input_queue, output_queue)| {
                input_queue.has_used_slots() || output_queue.has_used_slots()
            });
        debug_assert_eq!(Ok(()), self.test_invariants());
    }

    /// Queries whether the deadline of any message in the pool has expired.
    ///
    /// Time complexity: `O(1)`.
    pub fn has_expired_deadlines(&self, current_time: Time) -> bool {
        self.pool.has_expired_deadlines(current_time)
    }

    /// Drops expired messages given a current time, enqueuing a reject response for
    /// own requests into the matching reverse queue (input or output).
    ///
    /// Updating the correct input queues schedule after enqueuing a reject response into a
    /// previously empty queue also requires the full set of local canisters to decide whether
    /// the destination canister was local or remote.
    ///
    /// Returns the number of messages that were timed out.
    pub fn time_out_messages(
        &mut self,
        current_time: Time,
        own_canister_id: &CanisterId,
        local_canisters: &BTreeMap<CanisterId, CanisterState>,
    ) -> usize {
        let expired_messages = self.pool.expire_messages(current_time);
        for (id, msg) in expired_messages.iter() {
            self.on_message_dropped(*id, msg, own_canister_id, local_canisters);
        }

        debug_assert_eq!(Ok(()), self.test_invariants());
        debug_assert_eq!(Ok(()), self.schedules_ok(own_canister_id, local_canisters));
        expired_messages.len()
    }

    /// Removes the largest best-effort message in the underlying pool. Returns
    /// `true` if a message was removed; `false` otherwise.
    ///
    /// Updates the stats for the dropped message and (where applicable) the
    /// generated response. `own_canister_id` and `local_canisters` are required
    /// to determine the correct input queue schedule to update (if applicable).
    pub fn shed_largest_message(
        &mut self,
        own_canister_id: &CanisterId,
        local_canisters: &BTreeMap<CanisterId, CanisterState>,
    ) -> bool {
        if let Some((id, msg)) = self.pool.shed_largest_message() {
            self.on_message_dropped(id, &msg, own_canister_id, local_canisters);

            debug_assert_eq!(Ok(()), self.test_invariants());
            debug_assert_eq!(Ok(()), self.schedules_ok(own_canister_id, local_canisters));
            return true;
        }

        false
    }

    /// Handles the timing out or shedding of a message from the pool.
    ///
    /// Generates and enqueues a reject response if the message was an own request.
    /// And updates the stats for the dropped message and (where applicable) the
    /// generated response. `own_canister_id` and `local_canisters` are required
    /// to determine the correct input queue schedule to update (if applicable).
    fn on_message_dropped(
        &mut self,
        id: message_pool::Id,
        msg: &RequestOrResponse,
        own_canister_id: &CanisterId,
        local_canisters: &BTreeMap<CanisterId, CanisterState>,
    ) {
        use Context::*;

        let context = id.context();
        let remote = match context {
            Inbound => msg.sender(),
            Outbound => msg.receiver(),
        };
        let (input_queue, output_queue) = self.canister_queues.get_mut(&remote).unwrap();
        let (queue, reverse_queue) = match context {
            Inbound => (input_queue, output_queue),
            Outbound => (output_queue, input_queue),
        };

        // Ensure that the first reference in a queue is never stale: if we dropped the
        // message at the head of a queue, advance to the first non-stale reference.
        //
        // Defensive check, reference may have already been popped by an earlier
        // `on_message_dropped()` call if multiple messages got dropped at once.
        if queue.peek() == Some(&CanisterQueueItem::Reference(id)) {
            queue.pop();
            queue.pop_while(|item| self.pool.get(item.id()).is_none());
        }

        // Release the response slot, generate reject responses or remember shed inbound
        // responses, as necessary.
        match (context, msg) {
            // Inbound request: release the outbound response slot.
            (Inbound, RequestOrResponse::Request(request)) => {
                reverse_queue.release_reserved_response_slot();
                self.queue_stats.on_drop_input_request(request);
            }

            // Outbound request: enqueue a `SYS_TRANSIENT` timeout reject response.
            (Outbound, RequestOrResponse::Request(request)) => {
                let response = generate_timeout_response(request);
                let destination = &request.receiver;
                let (input_queue, _) = self.canister_queues.get_mut(destination).unwrap();

                // Update stats for the generated response.
                self.queue_stats.on_push_response(&response, Inbound);

                assert!(self
                    .callbacks_with_enqueued_response
                    .insert(response.originator_reply_callback));
                let id = self.pool.insert_inbound(response.into());
                input_queue.push_response(id);

                // If the input queue is not already in an input schedule, add it.
                if input_queue.len() == 1 && self.input_schedule_canisters.insert(remote) {
                    if &remote == own_canister_id || local_canisters.contains_key(&remote) {
                        self.local_subnet_input_schedule.push_back(remote)
                    } else {
                        self.remote_subnet_input_schedule.push_back(remote)
                    }
                }
            }

            (Inbound, RequestOrResponse::Response(response)) => {
                // TODO(MR-603): Recall the `Id` -> `CallbackId` of shed inbound responses and
                // generate a reject response on the fly when the respective `Id` is popped.
                assert!(self
                    .callbacks_with_enqueued_response
                    .remove(&response.originator_reply_callback));
            }

            // Outbound (best-effort) responses can be dropped with impunity.
            (Outbound, RequestOrResponse::Response(_)) => {}
        }
    }

    /// Re-partitions `self.local_subnet_input_schedule` and
    /// `self.remote_subnet_input_schedule` based on the set of all local canisters
    /// plus `own_canister_id` (since Rust's ownership rules would prevent us from
    /// mutating `self` if it was still under `local_canisters`).
    ///
    /// For use after a subnet split or other kind of canister migration. While an
    /// input queue that finds itself in the wrong schedule would get removed from
    /// said schedule as soon as it became empty (and would then get enqueued into
    /// the correct schedule), there is no guarantee that a given queue will ever
    /// become empty. Because of that, we explicitly re-partition schedules during
    /// canister migrations.
    pub(crate) fn split_input_schedules(
        &mut self,
        own_canister_id: &CanisterId,
        local_canisters: &BTreeMap<CanisterId, CanisterState>,
    ) {
        let local_schedule = std::mem::take(&mut self.local_subnet_input_schedule);
        let remote_schedule = std::mem::take(&mut self.remote_subnet_input_schedule);

        for canister_id in local_schedule.into_iter().chain(remote_schedule) {
            if &canister_id == own_canister_id || local_canisters.contains_key(&canister_id) {
                self.local_subnet_input_schedule.push_back(canister_id);
            } else {
                self.remote_subnet_input_schedule.push_back(canister_id);
            }
        }

        debug_assert_eq!(Ok(()), self.schedules_ok(own_canister_id, local_canisters));
    }

    /// Helper function to concisely validate `CanisterQueues`' input schedules
    /// during deserialization; or in debug builds, by writing
    /// `debug_assert_eq!(self.schedules_ok(own_canister_id, local_canisters).is_ok())`.
    ///
    /// Checks that all canister IDs of input queues that contain at least one message
    /// are found exactly once in either the input schedule for the local subnet or the
    /// input schedule for remote subnets.
    ///
    /// Time complexity: `O(n * log(n))`.
    fn schedules_ok(
        &self,
        own_canister_id: &CanisterId,
        local_canisters: &BTreeMap<CanisterId, CanisterState>,
    ) -> Result<(), String> {
        let mut local_schedule: HashSet<_> = self.local_subnet_input_schedule.iter().collect();
        let mut remote_schedule: HashSet<_> = self.remote_subnet_input_schedule.iter().collect();

        if local_schedule.len() != self.local_subnet_input_schedule.len()
            || remote_schedule.len() != self.remote_subnet_input_schedule.len()
            || local_schedule.intersection(&remote_schedule).count() != 0
        {
            return Err(format!(
                "Duplicate entries in local and/or remote input schedules:\n  `local_subnet_input_schedule`: {:?}\n  `remote_subnet_input_schedule`: {:?}",
                self.local_subnet_input_schedule, self.remote_subnet_input_schedule,
            ));
        }

        if self.local_subnet_input_schedule.len() + self.remote_subnet_input_schedule.len()
            != self.input_schedule_canisters.len()
            || local_schedule
                .iter()
                .chain(remote_schedule.iter())
                .any(|canister_id| !self.input_schedule_canisters.contains(canister_id))
        {
            return Err(
                format!("Inconsistent input schedules:\n  `local_subnet_input_schedule`: {:?}\n  `remote_subnet_input_schedule`: {:?}\n  `input_schedule_canisters`: {:?}",
                self.local_subnet_input_schedule, self.remote_subnet_input_schedule, self.input_schedule_canisters)
            );
        }

        for (canister_id, (input_queue, _)) in self.canister_queues.iter() {
            if input_queue.len() == 0 {
                continue;
            }

            if canister_id == own_canister_id || local_canisters.contains_key(canister_id) {
                // Definitely a local canister.
                if !local_schedule.remove(canister_id) {
                    return Err(format!(
                        "Local canister with non-empty input queue ({:?}) absent from `local_subnet_input_schedule`",
                        canister_id
                    ));
                }
            } else {
                // Remote canister or deleted local canister. Check in both schedules.
                if !remote_schedule.remove(canister_id) && !local_schedule.remove(canister_id) {
                    return Err(format!(
                        "Canister with non-empty input queue ({:?}) absent from input schedules",
                        canister_id
                    ));
                }
            }
        }

        // Note that a currently empty input queue may have been enqueued into an input
        // schedule before all its messages expired or were shed.

        Ok(())
    }

    /// Helper function for concisely validating invariants other than those of
    /// input queue schedules (no stale references at queue front, valid stats)
    /// during deserialization; or in debug builds, by writing
    /// `debug_assert_eq!(Ok(()), self.test_invariants())`.
    ///
    /// Time complexity: `O(n * log(n))`.
    fn test_invariants(&self) -> Result<(), String> {
        // Invariant: all canister queues (input or output) are either empty or start
        // with a non-stale reference.
        for (canister_id, (input_queue, output_queue)) in self.canister_queues.iter() {
            canister_queue_ok(input_queue, &self.pool, canister_id)?;
            canister_queue_ok(output_queue, &self.pool, canister_id)?;
        }

        // Reserved slot stats match the actual number of reserved slots.
        let calculated_stats = Self::calculate_queue_stats(
            &self.canister_queues,
            self.queue_stats.guaranteed_response_memory_reservations,
            self.queue_stats
                .transient_stream_guaranteed_responses_size_bytes,
        );
        if self.queue_stats != calculated_stats {
            return Err(format!(
                "Inconsistent stats:\n  expected: {:?}\n  actual: {:?}",
                calculated_stats, self.queue_stats
            ));
        }

        Ok(())
    }

    /// Computes stats for the given canister queues. Used when deserializing and in
    /// `debug_assert!()` checks. Takes the number of memory reservations from the
    /// caller, as the queues have no need to track memory reservations, so it
    /// cannot be computed. Same with the size of guaranteed responses in streams.
    ///
    /// Time complexity: `O(canister_queues.len())`.
    fn calculate_queue_stats(
        canister_queues: &BTreeMap<CanisterId, (CanisterQueue, CanisterQueue)>,
        guaranteed_response_memory_reservations: usize,
        transient_stream_guaranteed_responses_size_bytes: usize,
    ) -> QueueStats {
        let (input_queues_reserved_slots, output_queues_reserved_slots) = canister_queues
            .values()
            .map(|(iq, oq)| (iq.reserved_slots(), oq.reserved_slots()))
            .fold((0, 0), |(acc0, acc1), (item0, item1)| {
                (acc0 + item0, acc1 + item1)
            });
        QueueStats {
            guaranteed_response_memory_reservations,
            input_queues_reserved_slots,
            output_queues_reserved_slots,
            transient_stream_guaranteed_responses_size_bytes,
        }
    }
}

/// Pops and returns the item at the head of the queue and advances the queue
/// to the next non-stale item.
fn pop_and_advance(queue: &mut CanisterQueue, pool: &mut MessagePool) -> Option<RequestOrResponse> {
    let item = queue.pop()?;
    queue.pop_while(|item| pool.get(item.id()).is_none());

    let msg = pool.take(item.id());
    assert!(msg.is_some(), "stale reference at the head of queue");
    msg
}

/// Helper function for concisely validating the hard invariant that a canister
/// queuee is either empty of starts with a non-stale reference, by writing
/// `debug_assert_eq!(Ok(()), canister_queue_ok(...)`.
///
/// Time complexity: `O(log(n))`.
fn canister_queue_ok(
    queue: &CanisterQueue,
    pool: &MessagePool,
    canister_id: &CanisterId,
) -> Result<(), String> {
    if let Some(item) = queue.peek() {
        let id = item.id();
        if pool.get(id).is_none() {
            return Err(format!(
                "Stale reference at the head of {:?} queue to/from {}",
                id.context(),
                canister_id
            ));
        }
    }

    Ok(())
}

/// Generates a timeout reject response from a request, refunding its payment.
fn generate_timeout_response(request: &Arc<Request>) -> Response {
    Response {
        originator: request.sender,
        respondent: request.receiver,
        originator_reply_callback: request.sender_reply_callback,
        refund: request.payment,
        response_payload: Payload::Reject(RejectContext::new_with_message_length_limit(
            RejectCode::SysTransient,
            "Request timed out.",
            MR_SYNTHETIC_REJECT_MESSAGE_MAX_LEN,
        )),
        deadline: request.deadline,
    }
}

impl From<&CanisterQueues> for pb_queues::CanisterQueues {
    fn from(item: &CanisterQueues) -> Self {
        Self {
            ingress_queue: (&item.ingress_queue).into(),
            input_queues: Default::default(),
            output_queues: Default::default(),
            canister_queues: item
                .canister_queues
                .iter()
                .map(|(canid, (iq, oq))| CanisterQueuePair {
                    canister_id: Some(pb_types::CanisterId::from(*canid)),
                    input_queue: Some(iq.into()),
                    output_queue: Some(oq.into()),
                })
                .collect(),
            pool: if item.pool != MessagePool::default() {
                Some((&item.pool).into())
            } else {
                None
            },
            next_input_queue: ProtoNextInputQueue::from(&item.next_input_queue).into(),
            local_subnet_input_schedule: item
                .local_subnet_input_schedule
                .iter()
                .map(|canid| pb_types::CanisterId::from(*canid))
                .collect(),
            remote_subnet_input_schedule: item
                .remote_subnet_input_schedule
                .iter()
                .map(|canid| pb_types::CanisterId::from(*canid))
                .collect(),
            guaranteed_response_memory_reservations: item
                .queue_stats
                .guaranteed_response_memory_reservations
                as u64,
        }
    }
}

impl TryFrom<(pb_queues::CanisterQueues, &dyn CheckpointLoadingMetrics)> for CanisterQueues {
    type Error = ProxyDecodeError;
    fn try_from(
        (item, metrics): (pb_queues::CanisterQueues, &dyn CheckpointLoadingMetrics),
    ) -> Result<Self, Self::Error> {
        let mut canister_queues = BTreeMap::new();
        let mut pool = MessagePool::default();

        if !item.input_queues.is_empty() || !item.output_queues.is_empty() {
            // Backward compatibility: deserialize from `input_queues` and `output_queues`.

            if item.pool.is_some() || !item.canister_queues.is_empty() {
                return Err(ProxyDecodeError::Other(
                    "Both `input_queues`/`output_queues` and `pool`/`canister_queues` are populated"
                        .to_string(),
                ));
            }

            if item.input_queues.len() != item.output_queues.len() {
                return Err(ProxyDecodeError::Other(format!(
                    "CanisterQueues: Mismatched input ({}) and output ({}) queue lengths",
                    item.input_queues.len(),
                    item.output_queues.len()
                )));
            }
            for (ie, oe) in item
                .input_queues
                .into_iter()
                .zip(item.output_queues.into_iter())
            {
                if ie.canister_id != oe.canister_id {
                    return Err(ProxyDecodeError::Other(format!(
                        "CanisterQueues: Mismatched input {:?} and output {:?} queue entries",
                        ie.canister_id, oe.canister_id
                    )));
                }

                let canister_id = try_from_option_field(ie.canister_id, "QueueEntry::canister_id")?;
                let original_iq: queue::InputQueue =
                    try_from_option_field(ie.queue, "QueueEntry::queue")?;
                let original_oq: queue::OutputQueue =
                    try_from_option_field(oe.queue, "QueueEntry::queue")?;
                let iq = (original_iq, &mut pool).try_into()?;
                let oq = (original_oq, &mut pool).try_into()?;

                if canister_queues.insert(canister_id, (iq, oq)).is_some() {
                    metrics.observe_broken_soft_invariant(format!(
                        "CanisterQueues: Duplicate queues for canister {}",
                        canister_id
                    ));
                }
            }
        } else {
            pool = item.pool.unwrap_or_default().try_into()?;

            let mut enqueued_pool_messages = BTreeSet::new();
            canister_queues = item
                .canister_queues
                .into_iter()
                .map(|qp| {
                    let canister_id: CanisterId =
                        try_from_option_field(qp.canister_id, "CanisterQueuePair::canister_id")?;
                    let iq: CanisterQueue = try_from_option_field(
                        qp.input_queue.map(|q| (q, Context::Inbound)),
                        "CanisterQueuePair::input_queue",
                    )?;
                    let oq: CanisterQueue = try_from_option_field(
                        qp.output_queue.map(|q| (q, Context::Outbound)),
                        "CanisterQueuePair::output_queue",
                    )?;

                    iq.iter().chain(oq.iter()).for_each(|queue_item| {
                        if pool.get(queue_item.id()).is_some()
                            && !enqueued_pool_messages.insert(queue_item.id())
                        {
                            metrics.observe_broken_soft_invariant(format!(
                                "CanisterQueues: Message {:?} enqueued more than once",
                                queue_item.id()
                            ));
                        }
                    });

                    Ok((canister_id, (iq, oq)))
                })
                .collect::<Result<_, Self::Error>>()?;

            if enqueued_pool_messages.len() != pool.len() {
                metrics.observe_broken_soft_invariant(format!(
                    "CanisterQueues: Pool holds {} messages, but only {} of them are enqueued",
                    pool.len(),
                    enqueued_pool_messages.len()
                ));
            }
        }

        let queue_stats = Self::calculate_queue_stats(
            &canister_queues,
            item.guaranteed_response_memory_reservations as usize,
            0,
        );

        let next_input_queue = NextInputQueue::from(
            ProtoNextInputQueue::try_from(item.next_input_queue).unwrap_or_default(),
        );

        let mut local_subnet_input_schedule = VecDeque::new();
        for canister_id in item.local_subnet_input_schedule.into_iter() {
            local_subnet_input_schedule.push_back(canister_id.try_into()?);
        }
        let mut remote_subnet_input_schedule = VecDeque::new();
        for canister_id in item.remote_subnet_input_schedule.into_iter() {
            remote_subnet_input_schedule.push_back(canister_id.try_into()?);
        }
        let input_schedule_canisters = local_subnet_input_schedule
            .iter()
            .cloned()
            .chain(remote_subnet_input_schedule.iter().cloned())
            .collect();

        let callbacks_with_enqueued_response = canister_queues
            .values()
            .flat_map(|(input_queue, _)| input_queue.iter())
            .filter_map(|item| match pool.get(item.id()) {
                Some(RequestOrResponse::Response(rep)) => Some(rep.originator_reply_callback),
                _ => None,
            })
            .collect();

        let queues = Self {
            ingress_queue: IngressQueue::try_from(item.ingress_queue)?,
            canister_queues,
            pool,
            queue_stats,
            local_subnet_input_schedule,
            remote_subnet_input_schedule,
            input_schedule_canisters,
            next_input_queue,
            callbacks_with_enqueued_response,
        };

        // Safe to call with invalid `own_canister_id` and empty `local_canisters`, as
        // the validation logic allows for deleted local canisters.
        if let Err(e) = queues.schedules_ok(
            &CanisterId::unchecked_from_principal(PrincipalId::new_anonymous()),
            &BTreeMap::new(),
        ) {
            metrics.observe_broken_soft_invariant(e.to_string());
        }
        queues.test_invariants().map_err(ProxyDecodeError::Other)?;

        Ok(queues)
    }
}

/// Tracks slot and guaranteed response memory reservations across input and
/// output queues; and holds a (transient) byte size of responses already routed
/// into streams (tracked separately, at the replicated state level, as messages
/// are routed to and GC-ed from streams).
///
/// Stats for the enqueued messages themselves (counts and sizes by kind,
/// context and class) are tracked separately in `message_pool::MessageStats`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct QueueStats {
    /// Count of guaranteed response memory reservations across input and output
    /// queues. This is equivalent to the number of outstanding (inbound or outbound)
    /// guaranteed response calls and is used for computing message memory
    /// usage (as `MAX_RESPONSE_COUNT_BYTES` per request).
    ///
    /// Note that this is different from slots reserved for responses (whether
    /// best effort or guaranteed), which are used to implement backpressure.
    ///
    /// This is a counter maintained by `CanisterQueues` / `QueueStats`, but not
    /// computed from the queues themselves. Rather, it is validated against the
    /// number of unresponded guaranteed response callbacks and call contexts in the
    /// `CallContextManager`.
    guaranteed_response_memory_reservations: usize,

    /// Count of slots reserved in input queues. Note that this is different from
    /// memory reservations for guaranteed responses.
    input_queues_reserved_slots: usize,

    /// Count of slots reserved in output queues. Note that this is different from
    /// memory reservations for guaranteed responses.
    output_queues_reserved_slots: usize,

    /// Transient: size in bytes of guaranteed responses routed from `output_queues`
    /// into streams and not yet garbage collected.
    ///
    /// This is updated by `ReplicatedState::put_streams()`, called by MR after
    /// every streams mutation (induction, routing, GC). And is (re)populated during
    /// checkpoint loading by `ReplicatedState::new_from_checkpoint()`.
    transient_stream_guaranteed_responses_size_bytes: usize,
}

impl QueueStats {
    /// Returns the memory usage of reservations for guaranteed responses plus
    /// guaranteed responses in streans.
    pub fn guaranteed_response_memory_usage(&self) -> usize {
        self.guaranteed_response_memory_reservations * MAX_RESPONSE_COUNT_BYTES
            + self.transient_stream_guaranteed_responses_size_bytes
    }

    /// Updates the stats to reflect the enqueuing of the given message in the given
    /// context.
    fn on_push(&mut self, msg: &RequestOrResponse, context: Context) {
        match msg {
            RequestOrResponse::Request(request) => self.on_push_request(request, context),
            RequestOrResponse::Response(response) => self.on_push_response(response, context),
        }
    }

    /// Updates the stats to reflect the enqueuing of the given request in the given
    /// context.
    fn on_push_request(&mut self, request: &Request, context: Context) {
        // If pushing a guaranteed response request, make a memory reservation.
        if request.deadline == NO_DEADLINE {
            self.guaranteed_response_memory_reservations += 1;
        }

        if context == Context::Outbound {
            // If pushing a request into an output queue, reserve an input queue slot.
            self.input_queues_reserved_slots += 1;
        } else {
            // And the other way around.
            self.output_queues_reserved_slots += 1;
        }
    }

    /// Updates the stats to reflect the enqueuing of the given response in the
    /// given context.
    fn on_push_response(&mut self, response: &Response, context: Context) {
        // If pushing a guaranteed response, consume a memory reservation.
        if response.deadline == NO_DEADLINE {
            debug_assert!(self.guaranteed_response_memory_reservations > 0);
            self.guaranteed_response_memory_reservations = self
                .guaranteed_response_memory_reservations
                .saturating_sub(1);
        }

        if context == Context::Inbound {
            // If pushing a response into an input queue, consume an input queue slot.
            debug_assert!(self.input_queues_reserved_slots > 0);
            self.input_queues_reserved_slots = self.input_queues_reserved_slots.saturating_sub(1);
        } else {
            // And the other way around.
            debug_assert!(self.output_queues_reserved_slots > 0);
            self.output_queues_reserved_slots = self.output_queues_reserved_slots.saturating_sub(1);
        }
    }

    /// Updates the stats to reflect the dropping of the given request from an input
    /// queue.
    fn on_drop_input_request(&mut self, request: &Request) {
        // We should never be expiring or shedding a guaranteed response input request.
        debug_assert_ne!(NO_DEADLINE, request.deadline);

        debug_assert!(self.output_queues_reserved_slots > 0);
        self.output_queues_reserved_slots = self.output_queues_reserved_slots.saturating_sub(1);
    }
}

/// Checks whether `available_memory` for guaranteed response messages is
/// sufficient to allow enqueuing `msg` into an input or output queue.
///
/// Returns:
///  * `Ok(())` if `msg` is a best-effort message, as best-effort messages don't
///    consume guaranteed response memory.
///  * `Ok(())` if `msg` is a guaranteed `Response`, as guaranteed responses
///    always return memory.
///  * `Ok(())` if `msg` is a guaranteed response `Request` and
///    `available_memory` is sufficient.
///  * `Err(msg.count_bytes())` if `msg` is a guaranteed response `Request` and
///    `msg.count_bytes() > available_memory`.
pub fn can_push(msg: &RequestOrResponse, available_memory: i64) -> Result<(), usize> {
    match msg {
        RequestOrResponse::Request(req) => {
            let required = memory_required_to_push_request(req);
            if required as i64 <= available_memory || required == 0 {
                Ok(())
            } else {
                Err(required)
            }
        }
        RequestOrResponse::Response(_) => Ok(()),
    }
}

/// Returns the guaranteed response memory required to push `req` onto an input
/// or output queue.
///
/// For best-effort requests, this is always zero. For guaranteed response
/// requests, this is the maximum of `MAX_RESPONSE_COUNT_BYTES` (to be reserved
/// for a guaranteed response) and `req.count_bytes()` (if larger).
pub fn memory_required_to_push_request(req: &Request) -> usize {
    if req.deadline != NO_DEADLINE {
        return 0;
    }

    req.count_bytes().max(MAX_RESPONSE_COUNT_BYTES)
}

pub mod testing {
    use super::CanisterQueues;
    use crate::{InputQueueType, StateError};
    use ic_types::messages::{CanisterMessage, Request, RequestOrResponse};
    use ic_types::{CanisterId, Time};
    use std::collections::VecDeque;
    use std::sync::Arc;

    /// Exposes public testing-only `CanisterQueues` methods to be used in other
    /// crates' unit tests.
    pub trait CanisterQueuesTesting {
        /// Returns the number of messages in `ingress_queue`.
        fn ingress_queue_size(&self) -> usize;

        /// Pops the next message from the output queue associated with
        /// `dst_canister`.
        fn pop_canister_output(&mut self, dst_canister: &CanisterId) -> Option<RequestOrResponse>;

        /// Returns the number of output queues, empty or not.
        fn output_queues_len(&self) -> usize;

        /// Returns the number of messages in `output_queues`.
        fn output_message_count(&self) -> usize;

        /// Publicly exposes `CanisterQueues::push_input()`.
        fn push_input(
            &mut self,
            msg: RequestOrResponse,
            input_queue_type: InputQueueType,
        ) -> Result<(), (StateError, RequestOrResponse)>;

        /// Publicly exposes `CanisterQueues::pop_input()`.
        fn pop_input(&mut self) -> Option<CanisterMessage>;

        /// Publicly exposes the local subnet input_schedule.
        fn get_local_subnet_input_schedule(&self) -> &VecDeque<CanisterId>;

        /// Publicly exposes the remote subnet input_schedule.
        fn get_remote_subnet_input_schedule(&self) -> &VecDeque<CanisterId>;

        /// Returns an iterator over the raw contents of the output queue to
        /// `canister_id`; or `None` if no such output queue exists.
        fn output_queue_iter_for_testing(
            &self,
            canister_id: &CanisterId,
        ) -> Option<impl Iterator<Item = RequestOrResponse>>;
    }

    impl CanisterQueuesTesting for CanisterQueues {
        fn ingress_queue_size(&self) -> usize {
            self.ingress_queue.size()
        }

        fn pop_canister_output(&mut self, dst_canister: &CanisterId) -> Option<RequestOrResponse> {
            let queue = &mut self.canister_queues.get_mut(dst_canister).unwrap().1;
            super::pop_and_advance(queue, &mut self.pool)
        }

        fn output_queues_len(&self) -> usize {
            self.canister_queues.len()
        }

        fn output_message_count(&self) -> usize {
            self.pool.message_stats().outbound_message_count
        }

        fn push_input(
            &mut self,
            msg: RequestOrResponse,
            input_queue_type: InputQueueType,
        ) -> Result<(), (StateError, RequestOrResponse)> {
            self.push_input(msg, input_queue_type)
        }

        fn pop_input(&mut self) -> Option<CanisterMessage> {
            self.pop_input()
        }

        fn get_local_subnet_input_schedule(&self) -> &VecDeque<CanisterId> {
            &self.local_subnet_input_schedule
        }

        fn get_remote_subnet_input_schedule(&self) -> &VecDeque<CanisterId> {
            &self.remote_subnet_input_schedule
        }

        fn output_queue_iter_for_testing(
            &self,
            canister_id: &CanisterId,
        ) -> Option<impl Iterator<Item = RequestOrResponse>> {
            self.canister_queues
                .get(canister_id)
                .map(|(_, output_queue)| {
                    output_queue
                        .iter()
                        .filter_map(|item| self.pool.get(item.id()).cloned())
                })
        }
    }

    #[allow(dead_code)]
    /// Produces a `CanisterQueues` with requests enqueued in output queues,
    /// together with a `VecDeque` of raw requests, in the order in which they would
    /// be returned by `CanisterOutputQueuesIterator`.
    pub fn new_canister_output_queues_for_test(
        requests: Vec<Request>,
        sender: CanisterId,
        num_receivers: usize,
    ) -> (CanisterQueues, VecDeque<RequestOrResponse>) {
        let mut canister_queues = CanisterQueues::default();
        let mut updated_requests = VecDeque::new();
        requests.into_iter().enumerate().for_each(|(i, mut req)| {
            req.sender = sender;
            req.receiver = CanisterId::from_u64((i % num_receivers) as u64);
            let req = Arc::new(req);
            updated_requests.push_back(RequestOrResponse::Request(Arc::clone(&req)));
            canister_queues
                .push_output_request(req, Time::from_nanos_since_unix_epoch(i as u64))
                .unwrap();
        });
        (canister_queues, updated_requests)
    }
}
