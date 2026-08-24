image := "virtual-lb"

# run all local CI checks: compile check, tests, formatting, and lints
ci:
    cargo check --frozen
    cargo nextest run --no-tests=warn
    cargo fmt -- --check
    cargo clippy -- -D warnings
    cargo clippy -- -W clippy::pedantic

# build the operator's container image
docker-build:
    docker build -t {{ image }} .
