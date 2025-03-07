/*
for development testing i've been using:
    just ham-test
*/

use std::{net::Ipv4Addr, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use ham::{Ham, HamState};
use holochain_client::{AppWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner};
use holochain_conductor_api::CellInfo;
use holochain_types::{
    prelude::{ExternIO, GrantedFunctions},
    websocket::AllowedOrigins,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
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
    happ: String,

    /// Optional network seed for the app
    #[arg(short, long)]
    network_seed: Option<String>,
}

type ZomeName = String;
type ZomeCallFnName = String;
type MaybeZomeCallPayload = Option<String>;
type ZomeCalls = Vec<(ZomeName, (ZomeCallFnName, MaybeZomeCallPayload))>;

#[derive(Args)]
struct ZomeCallsCmdArgs {
    #[arg(long)]
    /// if provided, the path where the agent key will be persisted and read from on subsequent invocations
    agent_key_path: Option<PathBuf>,

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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Connect to the conductor
    let mut ham = Ham::connect(cli.port).await?;

    match cli.command {
        CliCommand::InstallAndInitHapp(install_and_init_happ_cmd_args) => {
            // Convert network_seed string to NetworkSeed if provided
            let network_seed = install_and_init_happ_cmd_args.network_seed;

            let mut ham_state_builder = ham::HamStateBuilder::default();

            let happ_bytes = Ham::get_happ_bytes(&install_and_init_happ_cmd_args.happ).await?;

            // Install and enable the app
            let (app_info, agent_key) = ham
                .install_and_enable_happ(&happ_bytes, network_seed.clone())
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
                anyhow::anyhow!("this command only works with an existing an valid state file")
            })?;

            let token_issued = ham
                .admin_ws
                .issue_app_auth_token(ham_state.app_info.installed_app_id.clone().into())
                .await
                .context("issuing token")?;

            // prepare the signer, which will receive the credentials for each cell in the subsequent loop
            let signer = ClientAgentSigner::default();

            let app_ws = AppWebsocket::connect(
                (Ipv4Addr::LOCALHOST, ham_state.app_ws_port),
                token_issued.token,
                signer.clone().into(),
            )
            .await
            .context("connecting to app websocket")?;

            for (cell_name, cell_infos) in ham_state.app_info.cell_info {
                // for each cell call the init zome function
                for cell_info in cell_infos {
                    println!("cell_info: {:#?}", &cell_info);

                    let cell_id = match &cell_info {
                        CellInfo::Provisioned(c) => c.cell_id.clone(),
                        CellInfo::Cloned(c) => c.cell_id.clone(),
                        other => anyhow::bail!("Invalid cell type: {other:?}"),
                    };

                    let credentials = ham
                        .admin_ws
                        // this writes a capgrand onto the source-chain to grant zomecall access to the `AgentPubKey` specified in the cell
                        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                            cell_id: cell_id.clone(),
                            functions: Some(GrantedFunctions::All),
                        })
                        .await
                        .context(format!(
                            "authorizing signing credentials for cell_id {}",
                            &cell_id,
                        ))?;
                    signer.add_credentials(cell_id.clone(), credentials);

                    'given_zome_calls: for (zome_name, (zome_fn_name, maybe_zome_fn_payload)) in
                        zome_calls_args.zome_calls.iter()
                    {
                        print!(
                            "[{cell_name}/{zome_name}] processing {zome_fn_name} @ {zome_name} with payload {maybe_zome_fn_payload:?}.. ",
                        );

                        let cell_name = match &cell_info {
                            CellInfo::Provisioned(provisioned_cell) => {
                                provisioned_cell.name.clone()
                            }
                            CellInfo::Cloned(cloned_cell) => cloned_cell.name.clone(),
                            CellInfo::Stem(stem_cell) => stem_cell.clone().name.unwrap_or_default(),
                        };
                        if !(zome_name.starts_with(&cell_name) || cell_name.starts_with(zome_name))
                        {
                            println!(
                                "skipping cell with name {cell_name} for call to zome {zome_name}"
                            );
                            continue 'given_zome_calls;
                        }

                        let payload = if let Some(payload) = maybe_zome_fn_payload.clone() {
                            ExternIO::encode(payload)
                        } else {
                            ExternIO::encode(())
                        }
                        .context("encoding payload")?;

                        match app_ws
                            .call_zome(
                                cell_id.clone().into(),
                                zome_name.clone().into(),
                                zome_fn_name.clone().into(),
                                payload,
                            )
                            .await
                            .map(|io| -> Result<Vec<String>, _> { io.decode() })
                        {
                            Ok(Ok(data)) => println!("success, got data:\n{data:#?}"),
                            Ok(Err(e)) => eprintln!("error: {e}"),
                            Err(e) => eprintln!("error: {e}"),
                        };
                    }
                }
            }
        }
    }

    Ok(())
}
