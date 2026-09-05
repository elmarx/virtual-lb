pub const MANAGER: &str = "virtual-lb";

/// lodBalancerClass
pub const VIRTUAL_LB_CLASS: &str = "athmer.cloud/virtual";
/// key for the label (on "members") and annotation (on the actual "virtual-lb")
pub const VIRTUAL_LB_NAME_KEY: &str = "athmer.cloud/virtual-lb.name";

pub mod selector {
    pub const VIRTUAL_LB_IS_MEMBER: &str = "athmer.cloud/virtual-lb.member=true";
    pub const TYPE_LB: &str = "spec.type=LoadBalancer";
}
