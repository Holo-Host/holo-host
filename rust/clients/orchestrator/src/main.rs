mod admin_client;
mod extern_api;
mod inventory;
mod utils;
mod workloads;
use anyhow::{Context, Result};
use db_utils::mongodb::get_mongodb_url;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nats_utils::jetstream_client::get_nats_url;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // Setup MongoDB Client
    let mongo_uri: String = get_mongodb_url();
    let db_client_options = ClientOptions::parse(&mongo_uri)
        .await
        .context(format!("mongo db client: connecting to {mongo_uri}"))?;
    let db_client = MongoDBClient::with_options(db_client_options)?;

    // TODO: Start Nats Auth Service (once ready)
    // let auth_client: Client = auth::run(db_client).await?;

    // Start Nats Admin Services
    log::debug!("spawning admin client...");
    let admin_client = admin_client::run(&None, get_nats_url()).await?;

    log::info!("Starting workload service...");
    if let Err(e) = workloads::run(admin_client.clone(), db_client.clone()).await {
        log::error!("Error running workload service. Err={:?}", e)
    };

    log::info!("Starting inventory service...");
    if let Err(e) = inventory::run(admin_client.clone(), db_client.clone()).await {
        log::error!("Error running inventory service. Err={:?}", e)
    };

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close admin client and drain internal buffer before exiting to make sure all messages are sent
    // NB: Calling drain/close on any one of the Client instances closes the underlying connection.
    // This affects all instances that share the same connection (including clones) because they are all references to the same resource.
    log::info!("Closing admin client...");
    admin_client.close().await?;

    // Close all mongodb connections
    log::debug!("Closing db connection...");
    db_client.shutdown().await;

    log::info!("Successfully shut down orchestrator");
    Ok(())
}
