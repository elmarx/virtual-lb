image := "cr.athmer.cloud/virtual-lb"
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
    docker build --push -t {{ image }} -t {{ image }}:{{ revision }} .
[working-directory: "manifests/deployment"]
kustomize_set_image:
    kustomize edit set image registry.athmer.cloud/virtual-lb=:{{ revision }}
    
