#![forbid(unsafe_code)]

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("hypo=info".parse().unwrap()))
        .init();

    // Step 8: clap CLI entry point (shell for now)
}
