use crate::context::Context;
use crate::errors;
use k8s_openapi::api::core::v1::Service;
use kube::runtime::controller::Action;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub async fn reconcile(
    service: Arc<Service>,
    _ctx: Arc<Context>,
) -> Result<Action, errors::VirtualLbError> {
    info!(service = ?service, "reconciling service");
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