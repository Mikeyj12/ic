use ic_protobuf::types::v1 as pb;
use ic_types::{
    artifact::{IdentifiableArtifact, IngressMessageId},
    batch::IngressPayload,
    consensus::{BlockPayload, BlockProposal, ConsensusMessage},
    CountBytes,
};

use super::types::stripped::{
    MaybeStrippedConsensusMessage, MaybeStrippedIngress, StrippedBlockProposal,
    StrippedIngressPayload,
};

/// If an ingress message has size above this threshold, we will strip it from the block.
const INGRESS_MESSAGE_SIZE_STRIPPING_THRESHOLD_BYTES: usize = 1024;

/// Provides functionality for stripping objects of given information.
///
/// For example, one might want to remove ingress messages from a block proposal.
pub(crate) trait Strippable {
    type Output;

    /// Strips ingress messages from the object.
    fn strip(self) -> Self::Output;
}

impl Strippable for ConsensusMessage {
    type Output = MaybeStrippedConsensusMessage;

    fn strip(self) -> Self::Output {
        match self {
            // We only strip data blocks.
            ConsensusMessage::BlockProposal(block_proposal)
                if block_proposal.as_ref().payload.payload_type()
                    == ic_types::consensus::PayloadType::Data =>
            {
                MaybeStrippedConsensusMessage::StrippedBlockProposal(block_proposal.strip())
            }
            msg => MaybeStrippedConsensusMessage::Unstripped(msg),
        }
    }
}

impl Strippable for BlockProposal {
    type Output = StrippedBlockProposal;

    fn strip(self) -> Self::Output {
        let unstripped_consensus_message_id = ConsensusMessage::BlockProposal(self.clone()).id();
        let mut proto = pb::BlockProposal::from(&self);

        // Remove the ingress payload from the proto.
        proto
            .value
            .as_mut()
            .map(|block| block.ingress_payload = None);

        let stripped_ingress_payload = match self.content.as_ref().payload.as_ref() {
            BlockPayload::Data(data) => data.batch.ingress.clone().strip(),
            // Summary block has no ingress messages
            BlockPayload::Summary(_) => StrippedIngressPayload {
                ingress_messages: vec![],
            },
        };

        Self::Output {
            block_proposal_without_ingresses_proto: proto,
            stripped_ingress_payload,
            unstripped_consensus_message_id,
        }
    }
}

impl Strippable for IngressPayload {
    type Output = StrippedIngressPayload;

    fn strip(self) -> Self::Output {
        let ingresses: Vec<_> = self.try_into().expect("FIXME");

        let stripped_ingresses = ingresses
            .into_iter()
            .map(|ingress| {
                let ingress_message_id = IngressMessageId::from(&ingress);

                if ingress.count_bytes() > INGRESS_MESSAGE_SIZE_STRIPPING_THRESHOLD_BYTES {
                    MaybeStrippedIngress::Stripped(ingress_message_id)
                } else {
                    MaybeStrippedIngress::Full(ingress_message_id, ingress)
                }
            })
            .collect();

        Self::Output {
            ingress_messages: stripped_ingresses,
        }
    }
}
