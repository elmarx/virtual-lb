mod constants;
mod context;
mod errors;
mod reconcile;
mod server;
mod service_ext;
mod telemetry;

use crate::constants::selector;
use crate::context::Context;
use crate::service_ext::ServiceExt;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::{Controller, watcher};
use kube::{Api, Client};
use server::Readiness;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

/// How often to re-poll the store's readiness while waiting for the initial sync.
///
/// `kube_runtime`'s `Store::wait_until_ready()` is backed by a single shared
/// oneshot channel: only the *most recently polled* waiter is guaranteed to be
/// woken when the store becomes ready. Since `Controller::run()` also awaits
/// the same store internally (to gate reconciliation), our independent call
/// here can lose the race and never get woken up, even though the store did
/// become ready. Re-polling on a short interval sidesteps this by not relying
/// on the wakeup at all.
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Waits for the controller's initial cache sync and flips the readiness flag.
///
/// If the sync never completes (e.g. the watcher's writer was dropped due to
/// an unrecoverable error), this is an unrecoverable startup failure: exit
/// immediately so Kubernetes restarts the pod, rather than leaving it stuck
/// forever behind a failing readiness probe.
async fn mark_ready_once_synced(
    store: kube::runtime::reflector::Store<Service>,
    readiness: Readiness,
) {
    loop {
        match tokio::time::timeout(READINESS_POLL_INTERVAL, store.wait_until_ready()).await {
            Ok(Ok(())) => {
                info!("controller cache synced, marking ready");
                readiness.set_ready();
                return;
            }
            Ok(Err(err)) => {
                error!(error = %err, "controller cache failed to synchronize, exiting");
                std::process::exit(1);
            }
            Err(_elapsed) => {
                // Still waiting (or our wakeup was lost to a concurrent waiter) - retry.
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init();

    info!("kuhbärnetes controller starting up");

    let addr: std::net::SocketAddr = "0.0.0.0:8080".parse().unwrap();

    let client = Client::try_default().await?;
    let readiness = Readiness::default();

    let ctx = Arc::new(Context {
        client: client.clone(),
    });

    let service_api = Api::<Service>::all(client.clone());
    let lb_watcher = watcher::Config::default().fields(selector::TYPE_LB);
    let virtual_lb_watcher = watcher::Config::default()
        .fields(selector::TYPE_LB)
        .labels(selector::VIRTUAL_LB_IS_MEMBER);

    let service_controller = Controller::new(service_api.clone(), lb_watcher);
    let primary_store = service_controller.store();
    let service_controller = service_controller
        .watches(service_api.clone(), virtual_lb_watcher, move |member| {
            // members are only relevant if they have the name set
            let Some(cluster_name) = member.virtual_lb_cluster_label() else {
                return Vec::new();
            };
            primary_store
                .state()
                .into_iter()
                .filter_map(|lb| {
                    if lb.is_our_load_balancer_class()
                        && lb.virtual_lb_cluster_annotation() == Some(cluster_name)
                    {
                        Some(ObjectRef::from_obj(lb.as_ref()))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .shutdown_on_signal();

    tokio::spawn(mark_ready_once_synced(
        service_controller.store(),
        readiness.clone(),
    ));

    let controller = service_controller
        .run(reconcile::reconcile, reconcile::error_policy, ctx)
        .for_each(|result| async move {
            if let Err(err) = result {
                error!(error = %err, "reconcile failed");
            }
        });

    let http_server = server::serve(addr, readiness, shutdown_signal());

    tokio::select! {
        () = controller => {
            error!("controller exited unexpectedly");
            return Err(anyhow::anyhow!("controller exited"));
        }
        result = http_server => {
            result.map_err(|err| anyhow::anyhow!("http server failed: {err}"))?;
        }
    }

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

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    tokio::select! {
        () = ctrl_c => info!("received SIGINT, shutting down"),
        () = terminate => info!("received SIGTERM, shutting down"),
    }
}
