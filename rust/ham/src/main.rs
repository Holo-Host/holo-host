/*
for development testing i've been using:
    just ham-test
*/

use std::{net::Ipv4Addr, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use ham::{Ham, HamState, ZomeCalls};
use holochain_client::AllowedOrigins;
use url::Url;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The port number of the Holochain conductor's admin interface
    #[arg(long, default_value = "127.0.0.1")]
    addr: Ipv4Addr,

    /// The port number of the Holochain conductor's admin interface
    #[arg(long, default_value = "4444")]
    port: u16,

    #[arg(long)]
    state_path: PathBuf,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Args)]
struct InstallAndInitHappCmdArgs {
    /// Path or URL to the .happ file to install
    #[arg(long)]
    happ: Url,

    /// Optional network seed for the app
    #[arg(short, long)]
    network_seed: Option<String>,
}

#[derive(Args)]
struct ZomeCallsCmdArgs {
    #[arg(long, value_parser = try_parse_zome_calls)]
    zome_calls: ZomeCalls,
}

fn try_parse_zome_calls(input: &str) -> Result<ZomeCalls> {
    let collection = input
        .split(",")
        .filter_map(|elem| {
            let mut split = elem.splitn(3, ":");

            let zome_name = split.next();
            let fn_name = split.next();
            let payload = split.next();

            match (zome_name, fn_name, payload) {
                (Some(zome_name), Some(fn_name), maybe_payload) => Some((
                    zome_name.to_string(),
                    (fn_name.to_string(), maybe_payload.map(ToString::to_string)),
                )),
                _ => {
                    eprintln!("WARNING: skipping incomplete element: {elem}");
                    None
                }
            }
        })
        .collect();
    Ok(collection)
}

#[derive(Subcommand)]
enum CliCommand {
    InstallAndInitHapp(InstallAndInitHappCmdArgs),
    ZomeCalls(ZomeCallsCmdArgs),
    FindInstalledApp { installed_app_id: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Connect to the conductor
    let mut ham = Ham::connect(cli.addr, cli.port).await?;

    match cli.command {
        CliCommand::InstallAndInitHapp(install_and_init_happ_cmd_args) => {
            // Convert network_seed string to NetworkSeed if provided
            let network_seed = install_and_init_happ_cmd_args.network_seed;

            let mut ham_state_builder = ham::HamStateBuilder::default();

            let happ_bytes = Ham::download_happ_bytes(&install_and_init_happ_cmd_args.happ).await?;

            // Install and enable the app
            let (app_info, agent_key) = ham
                .install_and_enable_happ(&happ_bytes, network_seed.clone(), None, None)
                .await
                .context(format!(
                    "installing happ from {} with network_seed {network_seed:?}",
                    install_and_init_happ_cmd_args.happ
                ))?;
            ham_state_builder = ham_state_builder.app_info(app_info);
            ham_state_builder = ham_state_builder.agent_key(agent_key);

            // Connect app agent client
            let app_ws_port = ham
                .admin_ws
                .attach_app_interface(0, AllowedOrigins::Any, None)
                .await
                .context("attaching app interface")?;
            ham_state_builder = ham_state_builder.app_ws_port(app_ws_port);

            let ham_state = ham_state_builder.build().context("building HamState")?;
            ham_state.persist(&cli.state_path)?;

            println!(
                "Successfully installed app {} and opened a app socket on port {app_ws_port}, and persisted state to {:?}",
                ham_state.app_info.installed_app_id,
                &cli.state_path,
            );
        }

        CliCommand::ZomeCalls(zome_calls_args) => {
            let ham_state = HamState::from_state_file(&cli.state_path)?.ok_or_else(|| {
                anyhow::anyhow!("this command only works with an existing and valid state file")
            })?;

            let results = ham
                .call_zomes(ham_state, zome_calls_args.zome_calls)
                .await?;

            println!("results: {results:#?}");
        }
        CliCommand::FindInstalledApp { installed_app_id } => {
            let results = ham.find_installed_app(&installed_app_id).await?;

            println!("results: {results:#?}");
        }
    }

    Ok(())
}
