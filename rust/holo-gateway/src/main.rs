use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting Holo Gateway Service");
    holo_gateway::run().await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;
    log::info!("Closing holo gateway nats client...");
    let nats_client = holo_gateway::types::nats::get_nats_client().await;
    nats_client.drain().await?;

    log::info!("Successfully shut down holo gateway service");
    Ok(())
}
