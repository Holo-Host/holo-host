use std::{env, sync::Arc};

use anyhow::Result;
use clap::Parser;
use nats_utils::{jetstream_client::JsClient, types::JsClientBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = holo_gateway::RunArgs::populate_from_environment();
    log::info!("Starting Holo Gateway Service with args");

    let nats_client = {
        let nats_url = args.nats_remote_args.nats_url.clone();
        let client = JsClient::new(JsClientBuilder {
            nats_remote_args: args.nats_remote_args.clone(),

            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url:?}: {e:?}"))
        .expect("need  a NATS client");

        Arc::new(client)
    };

    holo_gateway::run(Arc::clone(&nats_client), args).await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;
    log::info!("Closing holo gateway nats client...");
    let _ = nats_client
        .close()
        .await
        .inspect_err(|e| log::warn!("error closing NATS client: {e}"));

    log::info!("Successfully shut down holo gateway service");
    Ok(())
}
