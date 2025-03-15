mod admin_client;
mod extern_api;
mod inventory;
mod utils;
mod workloads;
use std::path::PathBuf;

use anyhow::{Context, Result};
use async_nats::ServerAddr;
use clap::Parser;
use db_utils::mongodb::get_mongodb_url;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use tokio::task::spawn;

#[derive(clap::Parser)]
struct Args {
    #[clap(long, env = "NATS_PASSWORD_FILE")]
    nats_password_file: Option<PathBuf>,

    #[clap(long, env = "NATS_USER")]
    nats_user: Option<String>,

    #[clap(long, env = "NATS_URL", default_value = "nats://127.0.0.1")]
    nats_url: ServerAddr,

    #[clap(
        long,
        default_value_t = false,
        env = "NATS_SKIP_TLS_VERIFICATION_DANGER"
    )]
    nats_skip_tls_verification_danger: bool,
}

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    let args = Args::parse();

    if args.nats_skip_tls_verification_danger {
        nats_utils::jetstream_client::tls_skip_verifier::early_in_process_install_crypto_provider();
    }

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
    let admin_client = admin_client::run(
        &None,
        &args.nats_url,
        args.nats_user,
        args.nats_password_file,
        args.nats_skip_tls_verification_danger,
    )
    .await?;

    let admin_workload_clone = admin_client.clone();
    let db_workload_clone = db_client.clone();
    spawn(async move {
        log::info!("Starting workload service...");
        if let Err(e) = workloads::run(admin_workload_clone, db_workload_clone).await {
            log::error!("Error running workload service. Err={:?}", e)
        };
    });

    let admin_inventory_clone = admin_client.clone();
    let db_inventory_clone = db_client.clone();
    spawn(async move {
        log::info!("Starting inventory service...");
        if let Err(e) = inventory::run(admin_inventory_clone, db_inventory_clone).await {
            log::error!("Error running inventory service. Err={:?}", e)
        };
    });

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
