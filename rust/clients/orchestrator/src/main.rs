mod admin_client;
mod extern_api;
mod inventory;
mod utils;
mod workloads;
use anyhow::Result;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use util_libs::db::mongodb::get_mongodb_url;

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // Setup MongoDB Client
    let mongo_uri: String = get_mongodb_url();
    let db_client_options = ClientOptions::parse(mongo_uri).await?;
    let db_client = MongoDBClient::with_options(db_client_options)?;

    // TODO: Start Nats Auth Service (once ready)
    // let auth_client: Client = auth::run(db_client).await?;

    // Start Nats Admin Services
    println!("spawning admin client...");
    let default_nats_connect_timeout_secs = 30;
    let admin_client = admin_client::run(&None, default_nats_connect_timeout_secs).await?;
    println!("starting workload service...");
    workloads::run(admin_client.clone(), db_client.clone()).await?;

    println!("starting inventory service...");
    inventory::run(admin_client.clone(), db_client.clone()).await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to close service gracefully");

    // Close all mongodb connections
    println!("closing db connection...");
    db_client.shutdown().await;

    // Close admin client and drain internal buffer before exiting to make sure all messages are sent
    // NB: Calling drain/close on any one of the Client instances closes the underlying connection.
    // This affects all instances that share the same connection (including clones) because they are all references to the same resource.
    println!("closing admin client...");
    admin_client.close().await?;
    log::debug!("Closed orchestrator auth service");

    Ok(())
}
