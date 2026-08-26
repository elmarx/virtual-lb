use crate::constants::{
    MANAGER, TYPE_LB_SELECTOR, VIRTUAL_LB_IS_MEMBER, VIRTUAL_LB_NAME_KEY,
};
use crate::context::Context;
use crate::errors;
use crate::service_ext::ServiceExt;
use k8s_openapi::api::core::v1::{LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus};
use kube::api::{ListParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub async fn reconcile(
    service: Arc<Service>,
    ctx: Arc<Context>,
) -> Result<Action, errors::VirtualLbError> {
    let name = service.name_any();
    // first check if this a gateway class we're responsible for
    if !service.is_our_load_balancer_class() {
        return Ok(Action::await_change());
    }

    let ns = service
        .namespace()
        .ok_or_else(|| errors::VirtualLbError::MissingNamespace(name.clone()))?;

    let Some(lb_name) = service.virtual_lb_cluster_annotation() else {
        warn!("service {name} is missing annotation {VIRTUAL_LB_NAME_KEY}");
        // TODO: write status into the service to indicate that this is an error
        return Ok(Action::await_change());
    };

    let service_api = Api::<Service>::namespaced(ctx.client.clone(), &ns);

    let list_parems = ListParams::default()
        .labels(&format!(
            "{VIRTUAL_LB_NAME_KEY}={lb_name},{VIRTUAL_LB_IS_MEMBER}=true"
        ))
        .fields(TYPE_LB_SELECTOR);
    let lb_members = service_api.list(&list_parems).await?;
    if lb_members.items.is_empty() {
        warn!("no members found for virtual loadbalancer {lb_name}");
        // TODO: write status, and is this the right action? we need to wait for the MEMBERs to change/be created
        return Ok(Action::await_change());
    }

    info!(
        "found {} members for virtual loadbalancer {lb_name}",
        lb_members.items.len()
    );

    let service_status = Service {
        status: Some(ServiceStatus {
            load_balancer: Some(LoadBalancerStatus {
                ingress: Some(vec![
                    LoadBalancerIngress {
                        ip_mode: Some("VIP".to_string()),
                        ip: Some("192.168.1.1".to_string()),
                        ..Default::default()
                    },
                    LoadBalancerIngress {
                        ip_mode: Some("VIP".to_string()),
                        ip: Some("192.168.1.2".to_string()),
                        ..Default::default()
                    },
                ]),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    service_api
        .patch_status(
            &name,
            &PatchParams::apply(MANAGER),
            &Patch::Apply(service_status),
        )
        .await?;

    info!("set loadbalancer for: {name}");
    Ok(Action::requeue(Duration::from_mins(1)))
}

pub fn error_policy(
    _service: Arc<Service>,
    err: &errors::VirtualLbError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(error = %err, "reconcile error");
    Action::requeue(Duration::from_secs(30))
}
