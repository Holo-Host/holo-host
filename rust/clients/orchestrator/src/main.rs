mod extern_api;
mod workloads;
use anyhow::Result;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();
    // Run auth service
    // TODO: invoke auth service (once ready)

    // Run workload service
    workloads::run().await?;

    Ok(())
}
