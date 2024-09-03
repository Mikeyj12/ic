use ic_protobuf::{
    proxy::{try_from_option_field, ProxyDecodeError},
    types::v1 as pb,
};
use ic_types::{
    artifact::{ConsensusMessageId, IdentifiableArtifact, IngressMessageId, PbArtifact},
    batch::{SelfValidatingPayload, ValidationContext, XNetPayload},
    consensus::{dkg, idkg, Block, BlockMetadata, ConsensusMessage, Rank},
    crypto::CryptoHashOf,
    messages::SignedIngress,
    signature::BasicSignature,
    Height, ReplicaVersion,
};

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum MaybeStrippedIngress {
    Full(IngressMessageId, SignedIngress),
    Stripped(IngressMessageId),
}

/// Stripped version of the [`IngressPayload`].
#[derive(Debug, Default)]
pub(crate) struct StrippedIngressPayload {
    pub(crate) ingress_messages: Vec<MaybeStrippedIngress>,
}

/// Stripped version of the [`DataPayload`].
#[derive(Debug)]
pub(crate) struct StrippedDataPayload {
    pub(crate) ingress: StrippedIngressPayload,
    //xnet: XNetPayload,
    //self_validating: SelfValidatingPayload,
    //canister_http: Vec<u8>,
    //query_stats: Vec<u8>,
    //dealings: dkg::Dealings,
    //idkg: idkg::Payload,
}

/// Stripped version of the [`BlockProposal`].
#[derive(Debug)]
pub struct StrippedBlockProposal {
    //pub(crate) version: ReplicaVersion,
    //pub(crate) parent: CryptoHashOf<Block>,
    pub(crate) payload: StrippedDataPayload,
    //pub(crate) height: Height,
    //pub(crate) rank: Rank,
    //pub(crate) context: ValidationContext,
    //pub(crate) unstripped_id: ConsensusMessageId,
    //pub(crate) block_hash: CryptoHashOf<Block>,
    //pub(crate) signature: BasicSignature<BlockMetadata>,
    pub(crate) unstripped_consensus_message_id: ConsensusMessageId,
}

#[derive(Debug)]
pub enum MaybeStrippedConsensusMessage {
    StrippedBlockProposal(StrippedBlockProposal),
    Unstripped(ConsensusMessage),
}

impl TryFrom<pb::StrippedConsensusMessage> for MaybeStrippedConsensusMessage {
    type Error = ProxyDecodeError;

    fn try_from(value: pb::StrippedConsensusMessage) -> Result<Self, Self::Error> {
        use pb::stripped_consensus_message::Msg;
        let Some(msg) = value.msg else {
            return Err(ProxyDecodeError::MissingField(
                "StrippedConsensusMessage::msg",
            ));
        };

        Ok(match msg {
            Msg::Unstripped(msg) => MaybeStrippedConsensusMessage::Unstripped(msg.try_into()?),
            // TODO(kpop): Implement this
            Msg::StrippedBlockProposal(_) => unimplemented!(),
        })
    }
}

impl From<MaybeStrippedConsensusMessage> for pb::StrippedConsensusMessage {
    fn from(value: MaybeStrippedConsensusMessage) -> Self {
        let msg = match value {
            MaybeStrippedConsensusMessage::Unstripped(unstripped) => {
                pb::stripped_consensus_message::Msg::Unstripped(unstripped.into())
            }
            MaybeStrippedConsensusMessage::StrippedBlockProposal(_) => todo!(),
        };

        Self { msg: Some(msg) }
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct StrippedConsensusMessageId(ConsensusMessageId);

impl AsRef<ConsensusMessageId> for StrippedConsensusMessageId {
    fn as_ref(&self) -> &ConsensusMessageId {
        &self.0
    }
}

impl From<StrippedConsensusMessageId> for pb::StrippedConsensusMessageId {
    fn from(value: StrippedConsensusMessageId) -> Self {
        pb::StrippedConsensusMessageId {
            unstripped_id: Some(value.0.into()),
        }
    }
}

impl TryFrom<pb::StrippedConsensusMessageId> for StrippedConsensusMessageId {
    type Error = ProxyDecodeError;

    fn try_from(value: pb::StrippedConsensusMessageId) -> Result<Self, Self::Error> {
        let unstripped = try_from_option_field(
            value.unstripped_id,
            "StrippedConsensusMessageId::unstripped_id",
        )?;

        Ok(Self(unstripped))
    }
}

impl IdentifiableArtifact for MaybeStrippedConsensusMessage {
    const NAME: &'static str = "strippedconsensus";

    type Id = StrippedConsensusMessageId;

    fn id(&self) -> Self::Id {
        let unstripped_id = match self {
            MaybeStrippedConsensusMessage::Unstripped(unstripped) => unstripped.id(),
            MaybeStrippedConsensusMessage::StrippedBlockProposal(stripped) => {
                stripped.unstripped_consensus_message_id.clone()
            }
        };

        StrippedConsensusMessageId(unstripped_id)
    }
}

impl PbArtifact for MaybeStrippedConsensusMessage {
    type PbId = pb::StrippedConsensusMessageId;

    type PbIdError = ProxyDecodeError;

    type PbMessage = pb::StrippedConsensusMessage;

    type PbMessageError = ProxyDecodeError;
}
