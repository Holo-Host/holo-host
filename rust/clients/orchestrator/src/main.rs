/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user

// This client is responsible for:
*/

mod auth;
mod utils;
mod workloads;
use anyhow::Result;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    let _ = auth::controller::run().await?;

    let _ = workloads::controller::run().await?;

    Ok(())
}
