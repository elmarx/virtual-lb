use crate::constants::{MANAGER, VIRTUAL_LB_NAME_KEY, selector};
use crate::context::Context;
use crate::service_ext::ServiceExt;
use crate::{errors, r#virtual};
use k8s_openapi::api::core::v1::Service;
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
            "{VIRTUAL_LB_NAME_KEY}={lb_name},{}",
            selector::VIRTUAL_LB_IS_MEMBER
        ))
        .fields(selector::TYPE_LB);
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

    let service_status = r#virtual::combine_members(&lb_members);

    let service = Service {
        status: Some(service_status),
        ..Default::default()
    };

    service_api
        .patch_status(&name, &PatchParams::apply(MANAGER), &Patch::Apply(service))
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
