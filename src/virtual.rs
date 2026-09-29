use crate::service_ext::ServiceExt;
use k8s_openapi::api::core::v1::{LoadBalancerStatus, Service, ServiceStatus};
use kube::api::ObjectList;

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

    let ingress = ingress
        .into_iter()
        .map(ToOwned::to_owned)
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
    use super::combine_members;
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
                hostname: Some("lb-1.example.com".to_owned()),
                ..Default::default()
            },
            LoadBalancerIngress {
                ip: Some("192.0.2.2".to_owned()),
                hostname: Some("lb-2.example.com".to_owned()),
                ..Default::default()
            },
        ];
        assert_eq!(actual, expected);
    }
}
