mod constants;
mod context;
mod errors;
mod reconcile;
mod server;
mod service_ext;
mod telemetry;

use crate::constants::{TYPE_LB_SELECTOR, VIRTUAL_LB_IS_MEMBER};
use crate::context::Context;
use crate::service_ext::ServiceExt;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::{Controller, watcher};
use kube::{Api, Client};
use server::Readiness;
use std::sync::Arc;
use tracing::{error, info};

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
    match store.wait_until_ready().await {
        Ok(()) => {
            info!("controller cache synced, marking ready");
            readiness.set_ready();
        }
        Err(err) => {
            error!(error = %err, "controller cache failed to synchronize, exiting");
            std::process::exit(1);
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
    let lb_watcher = watcher::Config::default().fields(TYPE_LB_SELECTOR);
    let virtual_lb_watcher = watcher::Config::default()
        .fields(TYPE_LB_SELECTOR)
        .labels(&format!("{VIRTUAL_LB_IS_MEMBER}=true"));

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
