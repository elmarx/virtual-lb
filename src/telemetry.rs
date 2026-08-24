//! Logging/tracing setup.
//!
//! Controlled via the standard `RUST_LOG` env var (e.g. `RUST_LOG=info,kuhbaernetes=debug`).
//! Set `LOG_FORMAT=json` to switch to structured JSON logs (useful in production
//! clusters where logs are shipped to something that parses JSON), otherwise a
//! human-friendly text format is used (nicer for local development).

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let json_output = std::env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json"));

    let registry = tracing_subscriber::registry().with(env_filter);

    if json_output {
        registry.with(fmt::layer().json().with_target(true)).init();
    } else {
        registry.with(fmt::layer().with_target(true)).init();
    }
}
