use clap::Parser;

/// Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL.
#[derive(Parser)]
#[clap(version)]
struct Flags {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .init();
    let _args = Flags::parse();
    Ok(())
}
