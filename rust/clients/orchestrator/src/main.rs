mod orchestrator;
mod config;
mod errors;
mod nats_clients;
mod auth;
mod admin;

use clap::Parser;
use dotenv::dotenv;
use nats_utils::jetstream_client::tls_skip_verifier::early_in_process_install_crypto_provider;
use nats_utils::types::NatsRemoteArgs;
use orchestrator::Orchestrator;
use errors::OrchestratorError;

// Re-export the Args struct from main.rs
#[derive(clap::Parser)]
pub struct Args {
    #[clap(flatten)]
    pub nats_remote_args: NatsRemoteArgs,
}


#[tokio::main]
async fn main() -> Result<(), OrchestratorError> {
    dotenv().ok();
    env_logger::init();

    let args = Args::parse();

    // Skip TLS  verification if requested
    if args.nats_remote_args.nats_skip_tls_verification_danger {
        early_in_process_install_crypto_provider();
    }

    // Create and run the orchestrator
    let orchestrator = Orchestrator::initialize(args).await?;
    orchestrator.run().await?;

    Ok(())
}