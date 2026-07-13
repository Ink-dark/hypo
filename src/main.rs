#![forbid(unsafe_code)]

use tracing_subscriber::EnvFilter;

fn main() {
    let filter = EnvFilter::from_default_env().add_directive(
        "hypo=info"
            .parse()
            .expect("内置 tracing 指令 'hypo=info' 解析失败，这不应发生"),
    );
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Step 8: clap CLI entry point (shell for now)
}
