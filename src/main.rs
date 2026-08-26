mod constants;
mod context;
mod errors;
mod reconcile;
mod server;
mod telemetry;

use crate::constants::{TYPE_LB_SELECTOR, VIRTUAL_LB_CLASS, VIRTUAL_LB_IS_MEMBER, VIRTUAL_LB_NAME};
use crate::context::Context;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::{Controller, watcher};
use kube::{Api, Client, ResourceExt};
use server::Readiness;
use std::sync::Arc;
use tracing::{error, info};

async fn mark_ready_once_synced(
    store: kube::runtime::reflector::Store<Service>,
    readiness: Readiness,
) {
    if store.wait_until_ready().await.is_ok() {
        info!("controller cache synced, marking ready");
        readiness.set_ready();
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
            let Some(cluster_name) = member.labels().get(VIRTUAL_LB_NAME) else {
                return Vec::new();
            };
            primary_store
                .state()
                .into_iter()
                .filter_map(|lb| {
                    // first check if this is our loadBalancerClass
                    let is_virtual_lb_class = lb
                        .spec
                        .as_ref()
                        .and_then(|s| s.load_balancer_class.as_ref())
                        .is_some_and(|lbc| lbc == VIRTUAL_LB_CLASS);

                    // now check if this is the virtual lb of the lb-cluster
                    let is_same_cluster = lb
                        .annotations()
                        .get(VIRTUAL_LB_NAME)
                        .is_some_and(|n| n == cluster_name);

                    if is_virtual_lb_class && is_same_cluster {
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
