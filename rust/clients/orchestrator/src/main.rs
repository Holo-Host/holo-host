mod admin_client;
mod auth;
mod extern_api;
mod inventory;
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

    // ==================== Setup MongoDB Client ====================
    let mongo_uri: String = get_mongodb_url();
    let db_client_options = ClientOptions::parse(mongo_uri).await?;
    let db_client = MongoDBClient::with_options(db_client_options)?;

    // ==================== Start Nats Auth Service ====================
    println!("starting auth...");
    let auth_client: Client = auth::run(db_client).await?;
    println!("finished setting up auth...");

    // ==================== Start Nats Admin Services ====================
    let db_client_clone = db_client.clone();
    spawn(async move {
        println!("spawning admin client...");
        if let Ok(admin_client) = admin_client::run().await {
            println!("starting workload service...");
            if let Err(e) = workloads::run(nats_client, db_client).await {
                log::error!("Error running workload service. Err={:?}", e)
            };

            println!("starting inventory service...");
            if let Err(e) = inventory::run(admin_client, db_client_clone).await {
                log::error!("Error running inventory service. Err={:?}", e)
            };

            // ==================== Close and Clean Admin Client ====================
            // Only exit program when explicitly requested
            tokio::signal::ctrl_c().await?;

            println!("closing admin client...");

            // Close client and drain internal buffer before exiting to make sure all messages are sent
            admin_client.close().await?;
        } else {
            log::error!(
                "Failed to spawn admin client and its dependant services (workload and inventory)."
            )
        }
    });

    // ==================== Close and Clean Auth Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    log::debug!("Closing orchestrator auth service...");

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    auth_client.drain().await?;
    log::debug!("Closed orchestrator auth service");

    Ok(())
}
