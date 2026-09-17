FROM rust:1.98-slim AS chef
WORKDIR /usr/src/virtual-lb
RUN cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /usr/src/virtual-lb/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian13:nonroot

LABEL org.opencontainers.image.title="virtual-lb" \
      org.opencontainers.image.description="Kubernetes controller for virtual LoadBalancer Services" \
      org.opencontainers.image.source="https://github.com/elmarx/virtual-lb"

WORKDIR /

COPY --from=builder /usr/src/virtual-lb/target/release/virtual-lb /

EXPOSE 8080
ENTRYPOINT ["/virtual-lb"]
