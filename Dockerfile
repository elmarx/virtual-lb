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

FROM debian:trixie-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home --shell /usr/bin/nologin virtual-lb

USER virtual-lb
WORKDIR /virtual-lb

COPY --from=builder /usr/src/virtual-lb/target/release/virtual-lb /usr/local/bin/virtual-lb

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/virtual-lb"]
