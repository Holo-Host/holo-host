/*
This client is associated with the:
  - HPOS account
  - host user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

pub mod agent_cli;
mod auth;
pub mod host_cmds;
mod hostd;
mod keys;
pub mod support_cmds;
use agent_cli::DaemonzeArgs;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentCliError {
    #[error("Agent Daemon Error")]
    AsyncNats(#[from] async_nats::Error),
    #[error("Command Line Error")]
    CommandError(#[from] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), AgentCliError> {
    dotenv().ok();
    env_logger::init();

    let cli = agent_cli::Root::parse();
    match &cli.scope {
        agent_cli::CommandScopes::Daemonize(daemonize_args) => {
            log::info!("Spawning host agent.");
            daemonize(daemonize_args).await?;
        }
        agent_cli::CommandScopes::Host { command } => host_cmds::host_command(command)?,
        agent_cli::CommandScopes::Support { command } => support_cmds::support_command(command)?,
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> Result<(), async_nats::Error> {
    let mut host_agent_keys = keys::Keys::try_from_storage(
        &args.nats_leafnode_client_creds_path,
        &args.nats_leafnode_client_sys_creds_path,
    )
    .or_else(|_| {
        keys::Keys::new().map_err(|e| {
            log::error!("Failed to create new keys: {:?}", e);
            async_nats::Error::from(e)
        })
    })?;

    // If user cred file is for the auth_guard user, run loop to authenticate host & hoster...
    if let keys::AuthCredType::Guard(_) = host_agent_keys.creds {
        host_agent_keys = auth::utils::run_auth_loop(host_agent_keys).await?;
    }

    log::trace!(
        "Host Agent Keys after successful authentication: {:#?}",
        host_agent_keys
    );

    // Once authenticated, start leaf server and run workload api calls.
    let bare_client = hostd::gen_leaf_server::run(
        &args.nats_leafnode_server_name,
        &host_agent_keys.get_host_creds_path(),
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
        args.nats_connect_timeout_secs,
    )
    .await?;
    // TODO: would it be a good idea to reuse this client in the workload_manager and elsewhere later on?
    bare_client.close().await?;

    let host_workload_client = hostd::workload::run(
        &host_agent_keys.host_pubkey,
        &host_agent_keys.get_host_creds_path(),
    )
    .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    host_workload_client.close().await?;

    Ok(())
}
