mod auth;
mod extern_api;
mod utils;
mod workloads;
use anyhow::Result;
use async_nats::Client;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use tokio::task::spawn;
use util_libs::db::mongodb::get_mongodb_url;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // Setup MongoDB Client
    let mongo_uri: String = get_mongodb_url();
    let db_client_options = ClientOptions::parse(mongo_uri).await?;
    let db_client = MongoDBClient::with_options(db_client_options)?;
    let thread_db_client = db_client.clone();

    // Start Nats Auth Service
    println!("starting auth...");
    let auth_client: Client = auth::run(db_client).await?;
    println!("finished setting up auth...");

    // Start Nats Admin Services
    spawn(async move {
        println!("spawning workload client...");
        let default_nats_connect_timeout_secs = 30;
        let admin_creds_path = None;
        if let Err(e) = workloads::run(
            thread_db_client,
            &admin_creds_path,
            default_nats_connect_timeout_secs,
        )
        .await
        {
            log::error!("Error running workload service. Err={:?}", e)
        }
    });

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    log::debug!("Closing orchestrator auth service...");

    // Close auth client and drain internal buffer before exiting to make sure all messages are sent
    auth_client.drain().await?;
    log::debug!("Closed orchestrator auth service");

    Ok(())
}
