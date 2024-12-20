/*
 This client is associated with the:
- WORKLOAD account
- hpos user

// This client is responsible for:
  - subscribing to workload streams
    - installing new workloads
    - removing workloads
    - send workload status upon request
  - sending active periodic workload reports
*/

// mod auth;
// mod utils;
mod workloads;
use anyhow::Result;
use dotenv::dotenv;
pub mod gen_leaf_server;
use util_libs::nats_js_client;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // let user_creds_path = auth::initializer::run().await?;
    let user_creds_path = "placeholder_creds_that_will_not_be_read".to_string();
    gen_leaf_server::run(&user_creds_path).await;

    let user_creds_path = nats_js_client::get_nats_client_creds("HOLO", "HPOS", "hpos");
    workloads::manager::run(&user_creds_path).await?;

    Ok(())
}
