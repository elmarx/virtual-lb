image := "cr.athmer.cloud/virtual-lb"
revision := `jj log -r @ --no-graph --template 'self.commit_id().short()'`

# run all local CI checks: compile check, tests, formatting, and lints
ci:
    cargo check --frozen
    cargo nextest run --no-tests=warn
    cargo fmt -- --check
    cargo clippy -- -D warnings
    cargo clippy -- -W clippy::pedantic

# build the operator's container image
build:
    docker build --push -t {{ image }} -t {{ image }}:{{ revision }} .
