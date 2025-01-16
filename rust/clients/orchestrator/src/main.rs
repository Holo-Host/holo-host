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
use tokio::task::spawn;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();
    spawn(async move { 
        if let Err(e) = auth::run().await {
            log::error!("{}", e)
        }
    });
    spawn(async move {
        if let Err(e) = workloads::run().await {
            log::error!("{}", e)
        } 
    });
    Ok(())
}
