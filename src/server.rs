//! Minimal HTTP server exposing the standard operator "sidecar" endpoints:
//!
//! - `/healthz` - liveness: process is up and the async runtime is responsive.
//! - `/readyz`  - readiness: set to `true` once the controller has done its
//!   initial cache sync (still a stub in this bare-bones commit; wired up once
//!   the controller/reconciler exists).
//! - `/metrics` - Prometheus scrape endpoint (placeholder for now; real
//!   reconcile metrics get registered here in a later commit).
//!
//! Kept deliberately framework-light (axum) so it's easy to extend per project.

use axum::{Router, http::StatusCode, routing::get};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use tracing::info;

/// Shared readiness flag. Clone this `Arc` into the controller setup and flip
/// it to `true` once the reconciler's initial relist/sync has completed.
#[derive(Clone, Default)]
pub struct Readiness(Arc<AtomicBool>);

impl Readiness {
    pub fn set_ready(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    fn is_ready(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub fn router(readiness: Readiness) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(move || readyz(readiness.clone())))
        .route("/metrics", get(metrics))
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

/// Kept `async` (despite not awaiting anything) because it's wrapped in a
/// closure passed to axum's `get()`, which requires the handler to return a
/// `Future` - a plain sync fn wouldn't satisfy axum's `Handler` trait here.
#[allow(clippy::unused_async)]
async fn readyz(readiness: Readiness) -> StatusCode {
    if readiness.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn metrics() -> &'static str {
    // Placeholder: wire up a real prometheus registry/exporter here once the
    // reconciler exists (reconcile counts, durations, error counts, ...).
    "# metrics not yet implemented\n"
}

/// Runs the health/metrics server until `shutdown` resolves.
pub async fn serve(
    addr: std::net::SocketAddr,
    readiness: Readiness,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "health/metrics server listening");
    axum::serve(listener, router(readiness))
        .with_graceful_shutdown(shutdown)
        .await
}
