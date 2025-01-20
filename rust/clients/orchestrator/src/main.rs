/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user
// This client is responsible for:
    - handling requests to add workloads
    - handling requests to update workloads
    - handling requests to remove workloads
    - handling workload status updates
    - interfacing with mongodb DB
*/

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
    if let Err(e) = workloads::run().await {
        log::error!("{}", e)
    }
    Ok(())
}
