//! Groups API definitions for standard gossip network requests/responses

use libp2p::{request_response::ResponseChannel, Multiaddr};
use serde::{Deserialize, Serialize};

use crate::gossip::types::WrappedPeerId;

use super::{
    cluster_management::ClusterJoinMessage, handshake::HandshakeMessage, hearbeat::HeartbeatMessage,
};

/// Represents an outbound gossip message, either a request to a peer
/// or a response to a peer's request
#[derive(Debug)]
pub enum GossipOutbound {
    /// A generic request sent to the network manager for outbound delivery
    Request {
        /// The PeerId of the peer sending the request
        peer_id: WrappedPeerId,
        /// The message contents in the request
        message: GossipRequest,
    },
    /// A generic response sent to the network manager for outbound delivery
    Response {
        /// The libp2p channel on which to send the response
        channel: ResponseChannel<GossipResponse>,
        /// The response body
        message: GossipResponse,
    },
    /// An outbound pubsub message to be flooded into the peer-to-peer network
    Pubsub {
        /// The topic being published to
        topic: String,
        /// The message contents
        message: PubsubMessage,
    },
    /// A command signalling to the network manager that a new node has been
    /// discovered at the application level. The network manager should register
    /// this node with the KDHT and propagate this change
    NewAddr {
        /// The PeerID to which the new address belongs
        peer_id: WrappedPeerId,
        /// The new address
        address: Multiaddr,
    },
}

/// Represents a request delivered point-to-point through the libp2p
/// request-response protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipRequest {
    /// A request from a peer initiating a heartbeat
    Heartbeat(HeartbeatMessage),
    /// A request from a peer initiating a handshake
    Handshake(HandshakeMessage),
}

/// Represents the possible response types for a request-response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipResponse {
    /// A response from a peer to a sender's heartbeat request
    Heartbeat(HeartbeatMessage),
    /// A response from a peer to a sender's handshake request
    Handshake(),
}

/// Represents a pubsub message flooded through the network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PubsubMessage {
    /// A message indicating that the publisher intends to join the given cluster
    Join(ClusterJoinMessage),
}

/// Explicit byte serialization and deserialization
///
/// libp2p gossipsub interface expects a type that can be cast
/// to and from bytes
impl From<PubsubMessage> for Vec<u8> {
    fn from(message: PubsubMessage) -> Self {
        serde_json::to_vec(&message).unwrap()
    }
}

impl From<Vec<u8>> for PubsubMessage {
    fn from(buf: Vec<u8>) -> Self {
        serde_json::from_slice(&buf).unwrap()
    }
}
