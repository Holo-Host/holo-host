/*
This client is associated with the:
  - WORKLOAD account
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
use hpos_hal::inventory::HoloInventory;
use thiserror::Error;
use util_libs::nats_js_client::{JsClient, PublishInfo};

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
    println!("inside host agent main auth... 0");

    let mut host_agent_keys = keys::Keys::try_from_storage(
        &args.nats_leafnode_client_creds_path,
        &args.nats_leafnode_client_sys_creds_path,
    )
    .or_else(|_| {
        keys::Keys::new().map_err(|e| {
            eprintln!("Failed to create new keys: {:?}", e);
            async_nats::Error::from(e)
        })
    })?;
    println!("Host Agent Keys={:#?}", host_agent_keys);

    // If user cred file is for the auth_guard user, run loop to authenticate host & hoster...
    if let keys::AuthCredType::Guard(_) = host_agent_keys.creds {
        host_agent_keys = run_auth_loop(host_agent_keys).await?;
    }

    println!(
        "Successfully AUTH'D and created new agent keys: {:#?}",
        host_agent_keys
    );

    // // Once authenticated, start leaf server and run workload api calls.
    // let _ = hostd::gen_leaf_server::run(
    //     &host_agent_keys.get_host_creds_path(),
    //     &args.store_dir,
    //     args.hub_url.clone(),
    //     args.hub_tls_insecure,
    // )
    // .await;

    // let host_workload_client = hostd::workloads::run(
    //     &host_agent_keys.host_pubkey,
    //     &host_agent_keys.get_host_creds_path(),
    //     args.nats_connect_timeout_secs,
    // )
    // .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // // Close client and drain internal buffer before exiting to make sure all messages are sent
    // host_workload_client.close().await?;

    Ok(())
}

async fn run_auth_loop(mut keys: keys::Keys) -> Result<keys::Keys, async_nats::Error> {
    let mut start = chrono::Utc::now();
    loop {
        log::debug!("About to run the Hosting Agent Authentication Service");
        let auth_guard_client: async_nats::Client;
        (keys, auth_guard_client) = auth::init::run(keys).await?;

        // If authenicated creds exist, then auth call was successful.
        // Close buffer, exit loop, and return.
        if let keys::AuthCredType::Authenticated(_) = keys.creds {
            auth_guard_client.drain().await?;
            break;
        }

        // Otherwise, send diagonostics every 1hr for the next 24hrs, then exit while loop and retry auth.
        // TODO: Discuss interval for sending diagnostic reports and wait duration before retrying auth with team.
        let now = chrono::Utc::now();
        let max_time_interval = chrono::TimeDelta::days(1);

        while max_time_interval > now.signed_duration_since(start) {
            let unauthenticated_user_diagnostics_subject =
                format!("DIAGNOSTICS.{}.unauthenticated", keys.host_pubkey);
            let diganostics = HoloInventory::from_host();
            let payload_bytes = serde_json::to_vec(&diganostics)?;

            if let Err(e) = auth_guard_client
                .publish(
                    unauthenticated_user_diagnostics_subject,
                    payload_bytes.into(),
                )
                .await
            {
                log::error!("Encountered error when sending diganostics. Err={:#?}", e);
            };
            tokio::time::sleep(chrono::TimeDelta::hours(1).to_std()?).await;
        }

        // Close and drain internal buffer before exiting to make sure all messages are sent.
        auth_guard_client.drain().await?;
        start = chrono::Utc::now();
    }

    Ok(keys)
}
