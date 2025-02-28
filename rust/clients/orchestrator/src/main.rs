mod auth;
mod extern_api;
mod utils;
mod workloads;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use async_nats::Client;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::str::FromStr;
use tokio::task::spawn;
use util_libs::db::mongodb::get_mongodb_url;
use util_libs::nats::{jetstream_client, types::Credentials};

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
    let admin_creds_path = PathBuf::from_str(&jetstream_client::get_nats_creds_by_nsc(
        "HOLO", "ADMIN", "admin",
    ))
    .map(Credentials::Path)
    .map_err(|e| anyhow!("Failed to locate admin credential path. Err={:?}", e))?;

    spawn(async move {
        println!("spawning workload client...");
        let nats_url = jetstream_client::get_nats_url();
        let default_nats_connect_timeout_secs = 30;
        let orchestrator_workload_client = match workloads::run(
            &nats_url,
            Some(admin_creds_path),
            default_nats_connect_timeout_secs,
            thread_db_client,
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                log::error!("Error running workload service. Err={:?}", e);
                return;
            }
        };

        // Only exit program when explicitly requested
        tokio::signal::ctrl_c()
            .await
            .context("Failed to handle ctrl-C key")
            .unwrap();

        // Close client and drain internal buffer before exiting to make sure all messages are sent
        orchestrator_workload_client
            .close()
            .await
            .expect("Failed to close orchestrator workload client");
    });

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    log::debug!("Closing orchestrator auth service...");

    // Close auth client and drain internal buffer before exiting to make sure all messages are sent
    auth_client.drain().await?;
    log::debug!("Closed orchestrator auth service");

    Ok(())
}
