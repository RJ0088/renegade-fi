//! The network manager handles lower level interaction with the p2p network

use crossbeam::channel::Sender;
use ed25519_dalek::{Keypair as SigKeypair, Signature, Signer, Verifier};
use futures::StreamExt;
use libp2p::{
    gossipsub::{GossipsubEvent, GossipsubMessage, Sha256Topic},
    identity::Keypair,
    request_response::{RequestResponseEvent, RequestResponseMessage},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::thread::JoinHandle;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver};
use tracing::{debug, event, Level};

use crate::{
    api::{
        cluster_management::ClusterManagementMessage,
        gossip::{
            GossipOutbound, GossipOutbound::Pubsub, GossipRequest, GossipResponse, PubsubMessage,
        },
    },
    gossip::{
        jobs::{ClusterManagementJob, GossipServerJob},
        types::{ClusterId, PeerInfo, WrappedPeerId},
    },
    handshake::jobs::HandshakeExecutionJob,
};

use super::{
    composed_protocol::{ComposedNetworkBehavior, ComposedProtocolEvent},
    error::NetworkManagerError,
    worker::NetworkManagerConfig,
};

/// Groups logic around monitoring and requesting the network
pub struct NetworkManager {
    /// The config passed from the coordinator thread
    pub(super) config: NetworkManagerConfig,
    /// The peerId of the locally running node
    pub(crate) local_peer_id: WrappedPeerId,
    /// The multiaddr of the local peer
    pub(crate) local_addr: Multiaddr,
    /// The cluster ID of the local perr
    pub(crate) cluster_id: ClusterId,
    /// The public key of the local peer
    pub(super) local_keypair: Keypair,
    /// The join handle of the executor loop
    pub(super) thread_handle: Option<JoinHandle<NetworkManagerError>>,
    /// The join handle of the cancellation relay
    pub(super) cancellation_relay_handle: Option<JoinHandle<NetworkManagerError>>,
}

/// The NetworkManager handles both incoming and outbound messages to the p2p network
/// It accepts events from workers elsewhere in the relayer that are to be propagated
/// out to the network; as well as listening on the network for messages from other peers.
impl NetworkManager {
    /// Setup global state after peer_id and address have been assigned
    pub(super) fn update_global_state_after_startup(&self) {
        // Add self to peer info index
        self.config.global_state.write_known_peers().insert(
            self.local_peer_id,
            PeerInfo::new(
                self.local_peer_id,
                self.cluster_id.clone(),
                self.local_addr.clone(),
            ),
        );

        // Add self to cluster metadata
        self.config
            .global_state
            .write_cluster_metadata()
            .add_member(self.local_peer_id);
    }

    /// Setup pubsub subscriptions for the network manager
    pub(super) fn setup_pubsub_subscriptions(
        &self,
        swarm: &mut Swarm<ComposedNetworkBehavior>,
    ) -> Result<(), NetworkManagerError> {
        // Cluster management topic for the local peer's cluster
        swarm
            .behaviour_mut()
            .pubsub
            .subscribe(&Sha256Topic::new(self.cluster_id.get_management_topic()))
            .map_err(|err| NetworkManagerError::SetupError(err.to_string()))?;

        Ok(())
    }

    /// The main loop in which the worker thread processes requests
    /// The worker handles two types of events:
    ///      1. Events from the network; which it dispatches to appropriate handler threads
    ///      2. Events from workers to be sent over the network
    /// It handles these in the tokio select! macro below
    pub(super) async fn executor_loop(
        local_peer_id: WrappedPeerId,
        cluster_key: SigKeypair,
        mut swarm: Swarm<ComposedNetworkBehavior>,
        mut send_channel: UnboundedReceiver<GossipOutbound>,
        gossip_work_queue: Sender<GossipServerJob>,
        handshake_work_queue: Sender<HandshakeExecutionJob>,
        mut cancel: Receiver<()>,
    ) -> NetworkManagerError {
        println!("Starting executor loop for network manager...");
        loop {
            tokio::select! {
                // Handle network requests from worker components of the relayer
                Some(message) = send_channel.recv() => {
                    // Forward the message
                    if let Err(err) = Self::handle_outbound_message(message, &cluster_key, &mut swarm) {
                        debug!("Error sending outbound message: {}", err.to_string());
                    }
                },

                // Handle network events and dispatch
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(event) => {
                            Self::handle_inbound_messsage(
                                event,
                                gossip_work_queue.clone(),
                                handshake_work_queue.clone()
                            )
                        },
                        SwarmEvent::NewListenAddr { address, .. } => {
                            println!("Listening on {}/p2p/{}\n", address, local_peer_id);
                        },
                        _ => {  }
                    }
                }

                // Handle a cancel signal from the coordinator
                _ = cancel.recv() => {
                    return NetworkManagerError::Cancelled("received cancel signal".to_string())
                }
            }
        }
    }

    /// Handles an outbound message from worker threads to other relayers
    fn handle_outbound_message(
        msg: GossipOutbound,
        cluster_key: &SigKeypair,
        swarm: &mut Swarm<ComposedNetworkBehavior>,
    ) -> Result<(), NetworkManagerError> {
        match msg {
            GossipOutbound::Request { peer_id, message } => {
                swarm
                    .behaviour_mut()
                    .request_response
                    .send_request(&peer_id, message);

                Ok(())
            }
            GossipOutbound::Response { channel, message } => swarm
                .behaviour_mut()
                .request_response
                .send_response(channel, message)
                .map_err(|_| {
                    NetworkManagerError::Network(
                        "error sending response, channel closed".to_string(),
                    )
                }),
            Pubsub { topic, mut message } => {
                // If the message is a cluster management message; the network manager should attach a signature
                #[allow(irrefutable_let_patterns)]
                if let PubsubMessage::ClusterManagement {
                    cluster_id,
                    message: body,
                    ..
                } = message
                {
                    // Sign the message with the cluster key
                    let signature = cluster_key
                        .sign(&Into::<Vec<u8>>::into(&body))
                        .to_bytes()
                        .to_vec();

                    message = PubsubMessage::ClusterManagement {
                        cluster_id,
                        signature,
                        message: body,
                    }
                }

                let topic = Sha256Topic::new(topic);
                swarm
                    .behaviour_mut()
                    .pubsub
                    .publish(topic, message)
                    .map_err(|err| NetworkManagerError::Network(err.to_string()))?;
                Ok(())
            }
            // Register a new peer in the distributed routing tables
            GossipOutbound::NewAddr { peer_id, address } => {
                swarm
                    .behaviour_mut()
                    .kademlia_dht
                    .add_address(&peer_id, address);

                Ok(())
            }
        }
    }

    /// Handles a network event from the relayer's protocol
    fn handle_inbound_messsage(
        message: ComposedProtocolEvent,
        gossip_work_queue: Sender<GossipServerJob>,
        handshake_work_queue: Sender<HandshakeExecutionJob>,
    ) {
        match message {
            ComposedProtocolEvent::RequestResponse(request_response) => {
                if let RequestResponseEvent::Message { peer, message } = request_response {
                    Self::handle_inbound_request_response_message(
                        peer,
                        message,
                        gossip_work_queue,
                        handshake_work_queue,
                    );
                }
            }
            // Pubsub events currently do nothing
            ComposedProtocolEvent::PubSub(msg) => {
                if let GossipsubEvent::Message { message, .. } = msg {
                    if let Err(err) =
                        Self::handle_inbound_pubsub_message(message, gossip_work_queue)
                    {
                        println!("Pubsub handler failed: {:?}", err);
                        event!(Level::ERROR, message = ?err, "error handling pubsub message");
                    }
                }
            }
            // KAD events do nothing for now, routing tables are automatically updated by libp2p
            ComposedProtocolEvent::Kademlia(_) => {}
        }
    }

    /**
     * Request/Response event handlers
     */

    /// Handle an incoming message from the network's request/response protocol
    fn handle_inbound_request_response_message(
        peer_id: PeerId,
        message: RequestResponseMessage<GossipRequest, GossipResponse>,
        gossip_work_queue: Sender<GossipServerJob>,
        handshake_work_queue: Sender<HandshakeExecutionJob>,
    ) {
        // Multiplex over request/response message types
        match message {
            // Handle inbound request from another peer
            RequestResponseMessage::Request {
                request, channel, ..
            } => match request {
                GossipRequest::Heartbeat(heartbeat_message) => {
                    gossip_work_queue
                        .send(GossipServerJob::HandleHeartbeatReq {
                            peer_id: WrappedPeerId(peer_id),
                            message: heartbeat_message,
                            channel,
                        })
                        .unwrap();
                }
                GossipRequest::Handshake(handshake_message) => {
                    handshake_work_queue
                        .send(HandshakeExecutionJob::ProcessHandshakeRequest {
                            peer_id: WrappedPeerId(peer_id),
                            message: handshake_message,
                            response_channel: channel,
                        })
                        .unwrap();
                }
                GossipRequest::Replicate(replicate_message) => {
                    gossip_work_queue
                        .send(GossipServerJob::Cluster(
                            ClusterManagementJob::ReplicateRequest(replicate_message),
                        ))
                        .unwrap();
                }
            },

            // Handle inbound response
            RequestResponseMessage::Response { response, .. } => match response {
                GossipResponse::Heartbeat(heartbeat_message) => {
                    gossip_work_queue
                        .send(GossipServerJob::HandleHeartbeatResp {
                            peer_id: WrappedPeerId(peer_id),
                            message: heartbeat_message,
                        })
                        .unwrap();
                }
                GossipResponse::Handshake() => {}
            },
        }
    }

    /**
     * Pubsub handlers
     */

    /// Handle an incoming network request for a pubsub message
    fn handle_inbound_pubsub_message(
        message: GossipsubMessage,
        gossip_work_queue: Sender<GossipServerJob>,
    ) -> Result<(), NetworkManagerError> {
        // Deserialize into API types
        let event: PubsubMessage = message.data.into();
        match event {
            PubsubMessage::ClusterManagement {
                cluster_id,
                signature,
                message,
            } => {
                // All cluster management messages are signed with the cluster private key for authentication
                // Parse the public key and signature from the payload
                let pubkey = cluster_id
                    .get_public_key()
                    .map_err(|err| NetworkManagerError::SerializeDeserialize(err.to_string()))?;
                let parsed_signature = Signature::from_bytes(&signature)
                    .map_err(|err| NetworkManagerError::SerializeDeserialize(err.to_string()))?;

                // Verify the signature
                pubkey
                    .verify(&Into::<Vec<u8>>::into(&message), &parsed_signature)
                    .map_err(|err| NetworkManagerError::Authentication(err.to_string()))?;

                // Forward the management message to the gossip server for processing
                match message {
                    ClusterManagementMessage::Join(join_request) => {
                        // Forward directly
                        gossip_work_queue
                            .send(GossipServerJob::Cluster(
                                ClusterManagementJob::ClusterJoinRequest(join_request),
                            ))
                            .map_err(|err| NetworkManagerError::EnqueueJob(err.to_string()))?;
                    }
                    ClusterManagementMessage::Replicated { wallets, peer_id } => {
                        // Forward one job per replicated wallet; makes gossip server implementation clenaer
                        for wallet in wallets.into_iter() {
                            gossip_work_queue
                                .send(GossipServerJob::Cluster(
                                    ClusterManagementJob::AddWalletReplica {
                                        wallet_id: wallet.wallet_id,
                                        peer_id,
                                    },
                                ))
                                .map_err(|err| NetworkManagerError::EnqueueJob(err.to_string()))?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
