use crate::service_ext::ServiceExt;
use k8s_openapi::api::core::v1::{LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus};
use kube::api::ObjectList;

/// detect if there is more than one hostname in the virtual load balancer group
fn conflicting_hostnames(ingress: &[&LoadBalancerIngress]) -> bool {
    let first_hostname = ingress.first().and_then(|i| i.hostname.as_deref());
    ingress
        .iter()
        .any(|i| i.hostname.as_deref() != first_hostname)
}

/// given a list of "real" LoadBalancer-Services, create a service-status that combines these service-ingresses into a new status
///
/// this is basically the "main logic" of this service
pub fn combine_members(lb_members: &ObjectList<Service>) -> ServiceStatus {
    let mut ingress = lb_members
        .iter()
        .flat_map(ServiceExt::ingress)
        .collect::<Vec<_>>();

    // sort (to avoid needless updates) and dedup
    ingress.sort_unstable_by_key(|k| (k.hostname.as_deref(), k.ip.as_deref()));
    ingress.dedup();

    // if we have conflicting hostnames, we have to strip the hostnames
    let strip_hostnames = conflicting_hostnames(&ingress);

    let ingress = ingress
        .into_iter()
        .map(|i| LoadBalancerIngress {
            hostname: if strip_hostnames {
                None
            } else {
                i.hostname.clone()
            },
            ip: i.ip.clone(),
            ip_mode: i.ip_mode.clone(),
            ports: i.ports.clone(),
        })
        .collect::<Vec<_>>();

    ServiceStatus {
        load_balancer: Some(LoadBalancerStatus {
            ingress: Some(ingress),
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{combine_members, conflicting_hostnames};
    use k8s_openapi::api::core::v1::{
        LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus,
    };
    use kube::api::{ListMeta, ObjectList, TypeMeta};

    fn service_with_ingress(ip: &str, hostname: &str) -> Service {
        Service {
            status: Some(ServiceStatus {
                load_balancer: Some(LoadBalancerStatus {
                    ingress: Some(vec![LoadBalancerIngress {
                        ip: Some(ip.to_owned()),
                        hostname: Some(hostname.to_owned()),
                        ..Default::default()
                    }]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn members(items: Vec<Service>) -> ObjectList<Service> {
        ObjectList {
            types: TypeMeta::list::<Service>(),
            metadata: ListMeta::default(),
            items,
        }
    }

    #[test]
    fn no_conflict_with_zero_or_one_ingress() {
        let ingress = LoadBalancerIngress {
            hostname: Some("lb.example.com".to_owned()),
            ..Default::default()
        };

        assert!(!conflicting_hostnames(&[]));
        assert!(!conflicting_hostnames(&[&ingress]));
    }

    #[test]
    fn no_conflict_with_matching_hostnames() {
        let first = LoadBalancerIngress {
            hostname: Some("lb.example.com".to_owned()),
            ..Default::default()
        };
        let second = LoadBalancerIngress {
            hostname: Some("lb.example.com".to_owned()),
            ip: Some("192.0.2.2".to_owned()),
            ..Default::default()
        };
        let missing = LoadBalancerIngress::default();

        assert!(!conflicting_hostnames(&[&first, &second]));
        assert!(!conflicting_hostnames(&[&missing, &missing]));
    }

    #[test]
    fn detects_conflicting_hostnames() {
        let first = LoadBalancerIngress {
            hostname: Some("lb-1.example.com".to_owned()),
            ..Default::default()
        };
        let second = LoadBalancerIngress {
            hostname: Some("lb-2.example.com".to_owned()),
            ..Default::default()
        };
        let missing = LoadBalancerIngress::default();

        assert!(conflicting_hostnames(&[&first, &second]));
        assert!(conflicting_hostnames(&[&first, &missing]));
        assert!(conflicting_hostnames(&[&missing, &first]));
    }

    #[test]
    fn combines_single_input() {
        let sample = members(vec![service_with_ingress("192.0.2.1", "lb-1.example.com")]);

        let actual = combine_members(&sample);
        let actual = actual.load_balancer.and_then(|lb| lb.ingress).unwrap();

        let expected = vec![LoadBalancerIngress {
            ip: Some("192.0.2.1".to_owned()),
            hostname: Some("lb-1.example.com".to_owned()),
            ..Default::default()
        }];

        assert_eq!(actual, expected);
    }

    #[test]
    fn combines_two_inputs() {
        let sample = members(vec![
            service_with_ingress("192.0.2.1", "lb-1.example.com"),
            service_with_ingress("192.0.2.2", "lb-2.example.com"),
        ]);

        let actual = combine_members(&sample);
        let actual = actual.load_balancer.and_then(|lb| lb.ingress).unwrap();

        let expected = vec![
            LoadBalancerIngress {
                ip: Some("192.0.2.1".to_owned()),
                hostname: None,
                ..Default::default()
            },
            LoadBalancerIngress {
                ip: Some("192.0.2.2".to_owned()),
                hostname: None,
                ..Default::default()
            },
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn preserves_shared_hostname() {
        let sample = members(vec![
            service_with_ingress("192.0.2.1", "lb.example.com"),
            service_with_ingress("192.0.2.2", "lb.example.com"),
        ]);

        let actual = combine_members(&sample);
        let actual = actual.load_balancer.and_then(|lb| lb.ingress).unwrap();

        let expected = vec![
            LoadBalancerIngress {
                ip: Some("192.0.2.1".to_owned()),
                hostname: Some("lb.example.com".to_owned()),
                ..Default::default()
            },
            LoadBalancerIngress {
                ip: Some("192.0.2.2".to_owned()),
                hostname: Some("lb.example.com".to_owned()),
                ..Default::default()
            },
        ];
        assert_eq!(actual, expected);
    }
}
