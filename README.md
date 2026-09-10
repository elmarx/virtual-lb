# virtual-lb

Kubernetes controller that implements a custom [LoadBalancer class](https://kubernetes.io/docs/concepts/services-networking/service/#loadbalancerclass) to aggregate multiple LoadBalancer services into a single virtual [LoadBalancer](https://kubernetes.io/docs/concepts/services-networking/service/#loadbalancer) by collecting their external IPs.

## Overview

virtual-lb groups existing LoadBalancer services under a common virtual LoadBalancer service using the custom `athmer.cloud/virtual-lb` [LoadBalancerClass](https://kubernetes.io/docs/concepts/services-networking/service/#loadbalancerclass). The virtual LoadBalancer's status reflects all external IPs from member services, enabling tools like [external-dns](https://kubernetes-sigs.github.io/external-dns) to register multiple addresses for a single DNS name.

## Motivation: IBM Kubernetes Service

In [IBM KS](https://www.ibm.com/de-de/products/kubernetes-service), each NLB (Network Load Balancer) [resides in a single zone](https://cloud.ibm.com/docs/vpc?topic=vpc-network-load-balancers) only. To achieve HA across zones, you must create separate LoadBalancer services for each [zone's NLB](https://cloud.ibm.com/docs/containers?topic=containers-setup_vpc_nlb#vpc_nlb_annotations). However, [external-dns](https://kubernetes-sigs.github.io/external-dns) will pick up each service independently, creating separate DNS records. virtual-lb solves this by aggregating multiple zone NLBs into a single virtual LoadBalancer that presents all zone IPs under one address.

## Installation

virtual-lb is available as a Helm chart in the OCI registry:

```bash
helm install virtual-lb oci://ghcr.io/elmarx/charts/virtual-lb
```

## Usage

1. **Label member services:**
   - Add labels to each LoadBalancer service you want to aggregate:
     - `athmer.cloud/virtual-lb.member: "true"`
     - `athmer.cloud/virtual-lb.name: <cluster-name>`

2. **Create virtual LoadBalancer service:**
   - Create a Service with `loadBalancerClass: athmer.cloud/virtual-lb`
   - Add annotation `athmer.cloud/virtual-lb.name: <cluster-name>`
   - Controller will populate its status with IPs from all matching member services
