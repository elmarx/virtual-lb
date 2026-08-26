use k8s_openapi::api::core::v1::LoadBalancerIngress;

use crate::constants::{VIRTUAL_LB_CLASS, VIRTUAL_LB_NAME_KEY};

pub trait ServiceExt {
    fn load_balancer_class(&self) -> Option<&str>;

    fn is_our_load_balancer_class(&self) -> bool;

    fn virtual_lb_cluster_annotation(&self) -> Option<&str>;

    fn virtual_lb_cluster_label(&self) -> Option<&str>;

    fn ingress(&self) -> impl Iterator<Item = &LoadBalancerIngress>;
}

impl ServiceExt for k8s_openapi::api::core::v1::Service {
    fn load_balancer_class(&self) -> Option<&str> {
        self.spec.as_ref()?.load_balancer_class.as_deref()
    }

    fn is_our_load_balancer_class(&self) -> bool {
        self.load_balancer_class()
            .is_some_and(|class| class == VIRTUAL_LB_CLASS)
    }

    fn virtual_lb_cluster_annotation(&self) -> Option<&str> {
        self.metadata
            .annotations
            .as_ref()?
            .get(VIRTUAL_LB_NAME_KEY)
            .map(String::as_str)
    }

    fn virtual_lb_cluster_label(&self) -> Option<&str> {
        self.metadata
            .labels
            .as_ref()?
            .get(VIRTUAL_LB_NAME_KEY)
            .map(String::as_str)
    }

    fn ingress(&self) -> impl Iterator<Item = &LoadBalancerIngress> {
        self.status
            .as_ref()
            .and_then(|s| s.load_balancer.as_ref())
            .and_then(|lb| lb.ingress.as_ref())
            .into_iter()
            .flatten()
    }
}
