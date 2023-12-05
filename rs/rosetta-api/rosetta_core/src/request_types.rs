use crate::identifiers::{NetworkIdentifier, PartialBlockIdentifier};
use crate::objects::Object;
use serde::{Deserialize, Serialize};

/// A MetadataRequest is utilized in any request where the only argument is
/// optional metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "conversion", derive(LabelledGeneric))]
pub struct MetadataRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Object>,
}

impl MetadataRequest {
    pub fn new() -> MetadataRequest {
        MetadataRequest { metadata: None }
    }
}

/// A NetworkRequest is utilized to retrieve some data specific exclusively to a NetworkIdentifier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetworkRequest {
    /// The network_identifier specifies which network a particular object is associated with.
    pub network_identifier: NetworkIdentifier,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Object>,
}

impl NetworkRequest {
    pub fn new(network_identifier: NetworkIdentifier) -> Self {
        Self {
            network_identifier,
            metadata: None,
        }
    }
}

/// A BlockRequest is utilized to make a block request on the /block endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "conversion", derive(LabelledGeneric))]
pub struct BlockRequest {
    /// The network_identifier specifies which network a particular object is associated with.
    pub network_identifier: NetworkIdentifier,

    /// When fetching data by BlockIdentifier, it may be possible to only specify the index or hash. If neither property is specified, it is assumed that the client is making a request at the current block.
    pub block_identifier: PartialBlockIdentifier,
}

impl BlockRequest {
    pub fn new(
        network_identifier: NetworkIdentifier,
        block_identifier: PartialBlockIdentifier,
    ) -> BlockRequest {
        BlockRequest {
            network_identifier,
            block_identifier,
        }
    }
}
