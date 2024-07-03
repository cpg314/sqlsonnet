use clap::Parser;
use tracing::*;

use clickhouse_proxy::{main_impl, Flags};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Flags::parse();
    if let Err(e) = main_impl(args).await {
        error!("{:?}", e);
        std::process::exit(1);
    }
    Ok(())
}
