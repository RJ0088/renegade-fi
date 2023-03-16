//! The core logic behind the APIServer's implementation

use std::{net::SocketAddr, sync::Arc};

use hyper::{
    server::{conn::AddrIncoming, Builder},
    Body, Request, Response,
};
use tokio::{
    net::TcpListener, runtime::Runtime, sync::mpsc::UnboundedSender as TokioSender,
    task::JoinHandle as TokioJoinHandle,
};

use crate::{
    price_reporter::jobs::PriceReporterManagerJob, system_bus::SystemBus, types::SystemBusMessage,
};

use super::{
    error::ApiServerError, router::Router, websocket::WebsocketHandler, worker::ApiServerConfig,
};

// ----------
// | Routes |
// ----------

/// Accepts inbound HTTP requests and websocket subscriptions and
/// serves requests from those connections
///
/// Clients of this server might be traders looking to manage their
/// trades, view live execution events, etc
pub struct ApiServer {
    /// The config passed to the worker
    pub(super) config: ApiServerConfig,
    /// The builder for the HTTP server before it begins serving; wrapped in
    /// an option to allow the worker threads to take ownership of the value
    pub(super) http_server_builder: Option<Builder<AddrIncoming>>,
    /// The join handle for the http server
    pub(super) http_server_join_handle: Option<TokioJoinHandle<ApiServerError>>,
    /// The join handle for the websocket server
    pub(super) websocket_server_join_handle: Option<TokioJoinHandle<ApiServerError>>,
    /// The tokio runtime that the http server runs inside of
    pub(super) server_runtime: Option<Runtime>,
}

impl ApiServer {
    /// The main execution loop for the websocket server
    pub(super) async fn websocket_execution_loop(
        addr: SocketAddr,
        price_reporter_worker_sender: TokioSender<PriceReporterManagerJob>,
        system_bus: SystemBus<SystemBusMessage>,
    ) -> Result<(), ApiServerError> {
        // Bind to the addr
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|err| ApiServerError::Setup(err.to_string()))?;

        // Loop over incoming streams
        while let Ok((stream, _)) = listener.accept().await {
            // Create a new handler on this stream
            let handler = WebsocketHandler::new(
                stream,
                price_reporter_worker_sender.clone(),
                system_bus.clone(),
            );
            tokio::spawn(handler.start());
        }

        // If the listener fails, the server has failed
        Err(ApiServerError::WebsocketServerFailure(
            "websocket server spuriously shutdown".to_string(),
        ))
    }

    /// Handles an incoming HTTP request
    pub(super) async fn handle_http_req(req: Request<Body>, router: Arc<Router>) -> Response<Body> {
        // Route the request
        router
            .handle_req(req.method().to_owned(), req.uri().path().to_string(), req)
            .await
    }
}
