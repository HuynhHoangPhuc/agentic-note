//! Prometheus metrics HTTP server initialization.
//!
//! Spawns a lightweight hyper HTTP server on localhost serving /metrics
//! in OpenMetrics text format. Returns a MetricsHandle for recording.

use crate::metrics_handle::MetricsHandle;
use hyper::body::Bytes;
use hyper::{Request, Response};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::sync::oneshot;

/// Start the Prometheus metrics HTTP server on the given port.
/// Returns the MetricsHandle and a shutdown sender.
/// When the shutdown sender is dropped or sent, the server stops.
pub async fn start_metrics_server(
    port: u16,
) -> anyhow::Result<(MetricsHandle, oneshot::Sender<()>)> {
    let handle = MetricsHandle::new();
    let metrics_handle = handle.clone();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Prometheus metrics server listening on http://{addr}/metrics");

    tokio::spawn(async move {
        let graceful = async {
            let _ = shutdown_rx.await;
        };

        tokio::select! {
            _ = serve_loop(listener, metrics_handle) => {}
            _ = graceful => {
                tracing::info!("Metrics server shutting down");
            }
        }
    });

    Ok((handle, shutdown_tx))
}

async fn serve_loop(listener: tokio::net::TcpListener, handle: MetricsHandle) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let handle = handle.clone();
        tokio::spawn(async move {
            let io = hyper_util::rt::TokioIo::new(stream);
            let service = hyper::service::service_fn(move |req| {
                let h = handle.clone();
                async move { handle_request(req, &h) }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await;
        });
    }
}

fn handle_request(
    req: Request<hyper::body::Incoming>,
    handle: &MetricsHandle,
) -> Result<Response<http_body_util::Full<Bytes>>, Infallible> {
    if req.uri().path() == "/metrics" {
        let body = handle.encode();
        Ok(Response::builder()
            .header("content-type", "application/openmetrics-text; version=1.0.0; charset=utf-8")
            .body(http_body_util::Full::new(Bytes::from(body)))
            .unwrap())
    } else {
        Ok(Response::builder()
            .status(404)
            .body(http_body_util::Full::new(Bytes::from("Not Found")))
            .unwrap())
    }
}

/// Install metrics recorder (no-op when metrics disabled).
/// Used for backwards compat — callers that don't need the HTTP server.
pub fn install_metrics_recorder(_port: u16) -> anyhow::Result<()> {
    tracing::info!("Metrics recording enabled (in-memory)");
    Ok(())
}
