mod admin_client;
mod auth;
mod extern_api;
mod inventory;
mod utils;
mod workloads;
use anyhow::Result;
use async_nats::Client;
use db_utils::mongodb::get_mongodb_url;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nats_utils::jetstream_client::get_nats_url;
use tokio::task::spawn;

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
    log::info!("Starting auth...");
    let auth_client: Client = auth::run(db_client.clone()).await?;
    log::debug!("Finished setting up auth...");

    // Start Nats Admin Services
    spawn(async move {
        log::debug!("Spawning admin client...");

        if let Ok(admin_client) = admin_client::run(&None, get_nats_url()).await {
            log::info!("Starting workload service...");

            if let Err(e) = workloads::run(admin_client.clone(), thread_db_client.clone()).await {
                log::error!("Error running workload service. Err={:?}", e)
            };

            log::info!("Starting inventory service...");
            if let Err(e) = inventory::run(admin_client.clone(), thread_db_client).await {
                log::error!("Error running inventory service. Err={:?}", e)
            };

            // Only exit program when explicitly requested
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to close service gracefully");

            // Close admin client and drain internal buffer before exiting to make sure all messages are sent
            // NB: Calling drain/close on any one of the Client instances closes the underlying connection.
            // This affects all instances that share the same connection (including clones) because they are all references to the same resource.
            log::info!("Closing admin client...");
            admin_client
                .close()
                .await
                .expect("Failed to close admin client gracefully");
        } else {
            log::error!(
                "Failed to spawn admin client and its dependant services (workload and inventory)."
            )
        }
    });

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close all mongodb connections
    log::debug!("Closing db connection...");
    db_client.shutdown().await;

    // Close auth client and drain internal buffer before exiting to make sure all messages are sent
    log::info!("Closing orchestrator auth service...");
    auth_client.drain().await?;

    log::info!("Successfully shut down orchestrator");
    Ok(())
}
