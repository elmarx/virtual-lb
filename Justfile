image := "virtual-lb"
revision := `jj log -r @ --no-graph --template 'self.commit_id().short()'`

# run all local CI checks: compile check, tests, formatting, and lints
ci:
    cargo check --locked
    cargo nextest run --no-tests=warn
    cargo fmt -- --check
    cargo clippy --all-features --all-targets -- -D warnings
    cargo clippy --all-features --all-targets -- -W clippy::pedantic

# build the operator's container image
build:
    docker build -t {{ image }} -t {{ image }}:{{ revision }} .

[working-directory("manifests/deployment")]
kustomize_set_image:
    kustomize edit set image virtual-lb=:{{ revision }}

# run a demo (in kind)
demo:
    #!/bin/sh
    set -eu

    [ -n "$(kind get clusters | grep virtual-lb)" ] || kind create cluster -n virtual-lb

    # check if demo namespace already exists
    if kubectl get namespace demo >/dev/null 2>&1; then
        echo "Error: namespace 'demo' already exists"
        exit 1
    fi

    kind load docker-image virtual-lb:latest --name virtual-lb
    kubectl apply -k ./manifests/deployment
    # since we're using image latest, make sure to update it
    kubectl rollout restart -n virtual-lb deployment virtual-lb

    kubectl create namespace demo

    # create the sample virtual LB
    kubectl apply -f ./manifests/example/lb.yaml

    # set up dummy LBs
    kubectl create -n demo svc loadbalancer a --tcp 8080
    kubectl create -n demo svc loadbalancer b --tcp 8080
    kubectl label -n demo svc a athmer.cloud/virtual-lb.member=true athmer.cloud/virtual-lb.name=demo
    kubectl label -n demo svc b athmer.cloud/virtual-lb.member=true athmer.cloud/virtual-lb.name=demo

    # "simulate" a loadbalancer setup by the cloud-provider
    kubectl patch -n demo svc a --subresource=status \
      -p '{"status":{"loadBalancer":{"ingress":[{"ip":"127.0.0.2"}]}}}'
    kubectl patch -n demo svc b --subresource=status \
      -p '{"status":{"loadBalancer":{"ingress":[{"ip":"127.0.0.3"}]}}}'

    # show virtual-lb
    kubectl get -n demo svc virtual

    # wait: this might not be necessary, because the reconcilation in the demo-cluster is almost instant, but it's correct
    kubectl wait -n demo --for=jsonpath='{.status.loadBalancer.ingress}' svc/virtual

    # show virtual-lb once again
    kubectl get -n demo svc virtual
