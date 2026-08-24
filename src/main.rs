mod server;
mod telemetry;

use server::Readiness;
use tracing::info;

const DEFAULT_HTTP_ADDR: &str = "0.0.0.0:8080";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init();

    info!("kuhbärnetes controller starting up");

    let addr: std::net::SocketAddr = std::env::var("HTTP_ADDR")
        .unwrap_or_else(|_| DEFAULT_HTTP_ADDR.to_string())
        .parse()
        .expect("HTTP_ADDR must be a valid socket address");

    let readiness = Readiness::default();
    // TODO: once a controller/reconciler is added, flip this only after the
    // initial watch/relist has completed instead of immediately.
    readiness.set_ready();

    server::serve(addr, readiness, shutdown_signal()).await?;

    info!("kuhbärnetes shut down cleanly");
    Ok(())
}

/// Resolves on SIGINT (Ctrl-C) or SIGTERM, whichever comes first, so the
/// process shuts down gracefully instead of being hard-killed by Kubernetes.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install SIGINT handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("received SIGINT, shutting down"),
        () = terminate => info!("received SIGTERM, shutting down"),
    }
}
