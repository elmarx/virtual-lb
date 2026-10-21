use crate::constants::{VIRTUAL_LB_NAME_KEY, selector};
use std::fmt;

/// name of a virtual load balancer group, linking the virtual-lb (annotation) to its members (label)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualLbName<'a>(&'a str);

impl<'a> VirtualLbName<'a> {
    pub fn new(name: &'a str) -> Self {
        Self(name)
    }

    /// label selector matching all members of this virtual load balancer
    pub fn member_selector(self) -> String {
        format!(
            "{VIRTUAL_LB_NAME_KEY}={},{}",
            self.0,
            selector::VIRTUAL_LB_IS_MEMBER
        )
    }
}

impl fmt::Display for VirtualLbName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
