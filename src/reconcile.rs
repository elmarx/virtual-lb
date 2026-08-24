use crate::context::Context;
use crate::errors;
use k8s_openapi::api::core::v1::{LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus};
use kube::{Api, Resource, ResourceExt};
use kube::runtime::controller::Action;
use std::sync::Arc;
use std::time::Duration;
use kube::api::{Patch, PatchParams};
use tracing::{info, warn};

pub const MANAGER: &str = "virtual-lb";


pub async fn reconcile(
    service: Arc<Service>,
    ctx: Arc<Context>,
) -> Result<Action, errors::VirtualLbError> {
    let name = service.name_any();
    // first check if this a gateway class we're responsible for
    let our_lb_class = service
        .spec
        .as_ref()
        .and_then(|spec| spec.load_balancer_class.as_ref())
        .is_some_and(|class| class == "athmer.cloud/virtual");
    if !our_lb_class {
        return Ok(Action::await_change());
    }

    let ns = service.namespace().ok_or_else(|| errors::VirtualLbError::MissingNamespace(name.clone()))?;

    let service_api = Api::<Service>::namespaced(ctx.client.clone(), &ns);

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
                    }
                ])}),
            ..Default::default()
        }),
        ..Default::default()
    };

    service_api.patch_status(
        &name, &PatchParams::apply(MANAGER), &Patch::Apply(service_status),
    ).await?;

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
