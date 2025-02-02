//! The entrypoint to the relayer, starts the coordinator thread which manages all other worker threads
#![feature(let_chains)]
#![feature(generic_const_exprs)]
#![feature(const_likely)]
#![allow(incomplete_features)]
#![deny(unsafe_code)]
#![deny(clippy::missing_docs_in_private_items)]

mod api_server;
mod chain_events;
mod config;
mod default_wrapper;
mod error;
mod external_api;
mod gossip;
mod gossip_api;
mod handshake;
mod network_manager;
mod price_reporter;
mod proof_generation;
mod starknet_client;
mod state;
mod system_bus;
mod types;
mod worker;

use std::{io::Write, process::exit, thread, time::Duration};

use chrono::Local;
use circuits::{types::wallet::Wallet, zk_gadgets::fixed_point::FixedPoint};
use crossbeam::channel;
use env_logger::Builder;
use error::CoordinatorError;
use gossip::worker::GossipServerConfig;
use handshake::worker::HandshakeManagerConfig;
use network_manager::worker::NetworkManagerConfig;
use num_bigint::BigUint;
use price_reporter::worker::PriceReporterManagerConfig;
use tokio::{
    select,
    sync::{
        mpsc,
        watch::{self, Receiver as WatchReceiver},
    },
};
use tracing::log::{self, LevelFilter};

use crate::{
    api_server::worker::{ApiServer, ApiServerConfig},
    chain_events::listener::{OnChainEventListener, OnChainEventListenerConfig},
    gossip::{jobs::GossipServerJob, server::GossipServer},
    gossip_api::gossip::GossipOutbound,
    handshake::{jobs::HandshakeExecutionJob, manager::HandshakeManager},
    network_manager::manager::NetworkManager,
    price_reporter::{jobs::PriceReporterManagerJob, manager::PriceReporterManager},
    proof_generation::{proof_manager::ProofManager, worker::ProofManagerConfig},
    starknet_client::client::{StarknetClient, StarknetClientConfig},
    state::RelayerState,
    system_bus::SystemBus,
    types::SystemBusMessage,
    worker::{watch_worker, Worker},
};

#[cfg(feature = "debug-tui")]
use crate::state::tui::StateTuiApp;

#[macro_use]
extern crate lazy_static;

/// A type alias for an empty channel used to signal cancellation to workers
pub(crate) type CancelChannel = WatchReceiver<()>;

// --------------------
// | Global Constants |
// --------------------

// TODO: Move these constants to a more discoverable location
lazy_static! {
    /// The fee the protocol takes on a match; one basis point
    static ref PROTOCOL_FEE: FixedPoint = FixedPoint::from_f32_round_down(0.0002);
    /// The public settle key of the protocol wallet
    /// Dummy value for now
    static ref PROTOCOL_SETTLE_KEY: BigUint = BigUint::from(0u8);
}

/// The system-wide value of MAX_BALANCES; the number of allowable balances a wallet holds
pub(crate) const MAX_BALANCES: usize = 5;
/// The system-wide value of MAX_ORDERS; the number of allowable orders a wallet holds
pub(crate) const MAX_ORDERS: usize = 5;
/// The system-wide value of MAX_FEES; the number of allowable fees a wallet holds
pub(crate) const MAX_FEES: usize = 2;
/// The height of the Merkle state tree used by the contract
pub(crate) const MERKLE_HEIGHT: usize = 32;
/// The number of historical roots the contract stores as being valid
pub(crate) const MERKLE_ROOT_HISTORY_LENGTH: usize = 30;
/// A type wrapper around the wallet type that adds the default generics above
pub(crate) type SizedWallet = Wallet<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
/// The amount of time to wait between sending teardown signals and terminating execution
const TERMINATION_TIMEOUT_MS: u64 = 10_000; // 10 seconds

// --------------
// | Entrypoint |
// --------------

/// The entrypoint to the relayer's execution
///
/// At a high level, this method beings a coordinator thread that:
///     1. Allocates resources and starts up workers
///     2. Watches worker threads for panics and errors
///     3. Cleans up and recovers any failed workers that are recoverable
///
/// The general flow for allocating a worker's resources is:
///     1. Allocate any communication primitives the worker needs access to (job queues, global bus, etc)
///     2. Build a cancel channel that the coordinator can use to cancel worker execution
///     3. Allocate and start the worker's execution
///     4. Allocate a thread to monitor the worker for faults
#[tokio::main]
async fn main() -> Result<(), CoordinatorError> {
    // ---------------------
    // | Environment Setup |
    // ---------------------

    // Parse command line arguments
    let args = config::parse_command_line_args().expect("error parsing command line args");
    let args_clone = args.clone();
    log::info!(
        "Relayer running with\n\t version: {}\n\t port: {}\n\t cluster: {:?}",
        args.version,
        args.p2p_port,
        args.cluster_id
    );

    // Build communication primitives
    // First, the global shared mpmc bus that all workers have access to
    let system_bus = SystemBus::<SystemBusMessage>::new();
    let (network_sender, network_receiver) = mpsc::unbounded_channel::<GossipOutbound>();
    let (gossip_worker_sender, gossip_worker_receiver) =
        mpsc::unbounded_channel::<GossipServerJob>();
    let (handshake_worker_sender, handshake_worker_receiver) =
        mpsc::unbounded_channel::<HandshakeExecutionJob>();
    let (price_reporter_worker_sender, price_reporter_worker_receiver) =
        mpsc::unbounded_channel::<PriceReporterManagerJob>();
    let (proof_generation_worker_sender, proof_generation_worker_receiver) = channel::unbounded();

    // Construct the global state and warm up the config orders by generating proofs of `VALID COMMITMENTS`
    let global_state = RelayerState::initialize_global_state(
        args.debug,
        args.wallets,
        args.cluster_id.clone(),
        system_bus.clone(),
    );

    // Configure logging and TUI
    #[cfg(feature = "debug-tui")]
    {
        if args.debug {
            // Build the TUI
            let tui = StateTuiApp::new(args_clone, global_state.clone());

            // Attach a watcher to the TUI and exit the process when the TUI quits
            let join_handle = tui.run();
            thread::spawn(move || {
                #[allow(unused_must_use)]
                join_handle.join();
                exit(0);
            });
        } else {
            configure_default_log_capture()
        }
    }

    #[cfg(not(feature = "debug-tui"))]
    {
        configure_default_log_capture();
    }

    // Spawn a thread to sync the relayer-global state with on-chain state and
    // network state
    global_state.initialize(
        args.contract_address.clone(),
        args.starknet_jsonrpc_node.clone().unwrap(),
        proof_generation_worker_sender.clone(),
        network_sender.clone(),
    );

    // ----------------
    // | Worker Setup |
    // ----------------

    // Construct a starknet client that workers will use to communicate with Starknet
    let starknet_client = StarknetClient::new(StarknetClientConfig {
        chain: args.chain_id,
        contract_addr: args.contract_address.clone(),
        infura_api_key: None,
        starknet_json_rpc_addr: args.starknet_jsonrpc_node.clone(),
        starknet_pkey: None,
    });

    // Start the network manager
    let (network_cancel_sender, network_cancel_receiver) = watch::channel(());
    let network_manager_config = NetworkManagerConfig {
        port: args.p2p_port,
        cluster_id: args.cluster_id.clone(),
        cluster_keypair: Some(args.cluster_keypair),
        send_channel: Some(network_receiver),
        gossip_work_queue: gossip_worker_sender.clone(),
        handshake_work_queue: handshake_worker_sender.clone(),
        global_state: global_state.clone(),
        cancel_channel: network_cancel_receiver,
    };
    let mut network_manager =
        NetworkManager::new(network_manager_config).expect("failed to build network manager");
    network_manager
        .start()
        .expect("failed to start network manager");

    let (network_failure_sender, mut network_failure_receiver) =
        mpsc::channel(1 /* buffer size */);
    watch_worker::<NetworkManager>(&mut network_manager, network_failure_sender);

    // Start the gossip server
    let (gossip_cancel_sender, gossip_cancel_receiver) = watch::channel(());
    let mut gossip_server = GossipServer::new(GossipServerConfig {
        local_peer_id: network_manager.local_peer_id,
        local_addr: network_manager.local_addr.clone(),
        cluster_id: args.cluster_id,
        bootstrap_servers: args.bootstrap_servers,
        starknet_client: starknet_client.clone(),
        global_state: global_state.clone(),
        job_sender: gossip_worker_sender.clone(),
        job_receiver: Some(gossip_worker_receiver).into(),
        network_sender: network_sender.clone(),
        cancel_channel: gossip_cancel_receiver,
    })
    .expect("failed to build gossip server");
    gossip_server
        .start()
        .expect("failed to start gossip server");
    let (gossip_failure_sender, mut gossip_failure_receiver) =
        mpsc::channel(1 /* buffer size */);
    watch_worker::<GossipServer>(&mut gossip_server, gossip_failure_sender);

    // Start the handshake manager
    let (handshake_cancel_sender, handshake_cancel_receiver) = watch::channel(());
    let mut handshake_manager = HandshakeManager::new(HandshakeManagerConfig {
        global_state: global_state.clone(),
        network_channel: network_sender.clone(),
        job_receiver: Some(handshake_worker_receiver),
        job_sender: handshake_worker_sender.clone(),
        proof_manager_sender: proof_generation_worker_sender.clone(),
        system_bus: system_bus.clone(),
        cancel_channel: handshake_cancel_receiver,
    })
    .expect("failed to build handshake manager");
    handshake_manager
        .start()
        .expect("failed to start handshake manager");
    let (handshake_failure_sender, mut handshake_failure_receiver) =
        mpsc::channel(1 /* buffer size */);
    watch_worker::<HandshakeManager>(&mut handshake_manager, handshake_failure_sender);

    // Start the price reporter manager
    let (price_reporter_cancel_sender, price_reporter_cancel_receiver) = watch::channel(());
    let mut price_reporter_manager = PriceReporterManager::new(PriceReporterManagerConfig {
        system_bus: system_bus.clone(),
        job_receiver: Some(price_reporter_worker_receiver).into(),
        cancel_channel: price_reporter_cancel_receiver,
        coinbase_api_key: args.coinbase_api_key,
        coinbase_api_secret: args.coinbase_api_secret,
        eth_websocket_addr: args.eth_websocket_addr,
    })
    .expect("failed to build price reporter manager");
    price_reporter_manager
        .start()
        .expect("failed to start price reporter manager");
    let (price_reporter_failure_sender, mut price_reporter_failure_receiver) =
        mpsc::channel(1 /* buffer size */);
    watch_worker::<PriceReporterManager>(
        &mut price_reporter_manager,
        price_reporter_failure_sender,
    );

    // Start the on-chain event listener
    let (chain_listener_cancel_sender, chain_listener_cancel_receiver) = watch::channel(());
    let mut chain_listener = OnChainEventListener::new(OnChainEventListenerConfig {
        starknet_client: starknet_client.clone(),
        global_state: global_state.clone(),
        handshake_manager_job_queue: handshake_worker_sender,
        proof_generation_work_queue: proof_generation_worker_sender.clone(),
        network_manager_work_queue: network_sender.clone(),
        cancel_channel: chain_listener_cancel_receiver,
    })
    .expect("failed to build on-chain event listener");
    chain_listener
        .start()
        .expect("failed to start on-chain event listener");
    let (chain_listener_failure_sender, mut chain_listener_failure_receiver) =
        mpsc::channel(1 /* buffer_size */);
    watch_worker::<OnChainEventListener>(&mut chain_listener, chain_listener_failure_sender);

    // Start the API server
    let (api_cancel_sender, api_cancel_receiver) = watch::channel(());
    let mut api_server = ApiServer::new(ApiServerConfig {
        http_port: args.http_port,
        websocket_port: args.websocket_port,
        global_state: global_state.clone(),
        system_bus,
        price_reporter_work_queue: price_reporter_worker_sender,
        proof_generation_work_queue: proof_generation_worker_sender,
        cancel_channel: api_cancel_receiver,
    })
    .expect("failed to build api server");
    api_server.start().expect("failed to start api server");
    let (api_failure_sender, mut api_failure_receiver) = mpsc::channel(1 /* buffer_size */);
    watch_worker::<ApiServer>(&mut api_server, api_failure_sender);

    // Start the proof generation module
    let (proof_manager_cancel_sender, proof_manager_cancel_receiver) = watch::channel(());
    let mut proof_manager = ProofManager::new(ProofManagerConfig {
        job_queue: proof_generation_worker_receiver,
        cancel_channel: proof_manager_cancel_receiver,
    })
    .expect("failed to build proof generation module");
    proof_manager
        .start()
        .expect("failed to start proof generation module");
    let (proof_manager_failure_sender, mut proof_manager_failure_receiver) =
        mpsc::channel(1 /* buffer_size */);
    watch_worker::<ProofManager>(&mut proof_manager, proof_manager_failure_sender);

    // For simplicity, we simply cancel all disabled workers, it is simpler to do this than work with
    // a dynamic list of futures
    //
    // We can refactor this decision if it becomes a performance issue
    if args.disable_api_server {
        api_server.cleanup().unwrap();
    }

    if args.disable_price_reporter {
        price_reporter_cancel_sender.send(()).unwrap();
    }

    // Await module termination, and send a cancel signal for any modules that
    // have been detected to fault
    let recovery_loop = || async {
        loop {
            select! {
                _ = network_failure_receiver.recv() => {
                    network_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    network_manager = recover_worker(network_manager)?;
                }
                _ = gossip_failure_receiver.recv() => {
                    gossip_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    gossip_server = recover_worker(gossip_server)?;
                }
                _ = handshake_failure_receiver.recv() => {
                    handshake_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    handshake_manager = recover_worker(handshake_manager)?;
                }
                _ = price_reporter_failure_receiver.recv() => {
                    price_reporter_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    price_reporter_manager = recover_worker(price_reporter_manager)?;
                }
                _= chain_listener_failure_receiver.recv() => {
                    chain_listener_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    chain_listener = recover_worker(chain_listener)?;
                }
                _ = api_failure_receiver.recv() => {
                    api_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    api_server = recover_worker(api_server)?;
                }
                _ = proof_manager_failure_receiver.recv() => {
                    proof_manager_cancel_sender.send(())
                        .map_err(|err| CoordinatorError::CancelSend(err.to_string()))?;
                    proof_manager = recover_worker(proof_manager)?;
                }
            };
        }
    };

    // Wait for an error, log the error, and teardown the relayer
    let loop_res: Result<(), CoordinatorError> = recovery_loop().await;
    let err = loop_res.err().unwrap();
    log::info!("Error in coordinator thread: {:?}", err);

    // Send cancel signals to all workers
    for cancel_channel in [
        network_cancel_sender,
        gossip_cancel_sender,
        handshake_cancel_sender,
        price_reporter_cancel_sender,
        chain_listener_cancel_sender,
        api_cancel_sender,
        proof_manager_cancel_sender,
    ]
    .iter()
    {
        cancel_channel.send(()).unwrap();
    }

    // Give workers time to teardown execution then terminate
    log::info!("Tearing down workers...");
    thread::sleep(Duration::from_millis(TERMINATION_TIMEOUT_MS));
    log::info!("Terminating...");

    Err(err)
}

/// Configures the default log capture which logs to stdout
fn configure_default_log_capture() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
}

/// Attempt to recover a failed module by cleaning up its resources and re-allocating it
fn recover_worker<W: Worker>(failed_worker: W) -> Result<W, CoordinatorError> {
    if !failed_worker.is_recoverable() {
        return Err(CoordinatorError::Recovery(format!(
            "worker {} is not recoverable",
            failed_worker.name()
        )));
    }

    Ok(failed_worker.recover())
}
