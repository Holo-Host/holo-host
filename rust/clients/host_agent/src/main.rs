/*
This client is associated with the:
  - WORKLOAD account
  - hpos user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

mod workload_manager;
use anyhow::Result;
use dotenv::dotenv;
pub mod gen_leaf_server;
use util_libs::nats_js_client;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();
    log::info!("Spawning host_agent");

    let user_creds_path = nats_js_client::get_nats_client_creds("HOLO", "HPOS", "hpos");

    gen_leaf_server::run(&user_creds_path).await;

    workload_manager::run(&user_creds_path).await?;

    Ok(())
}
