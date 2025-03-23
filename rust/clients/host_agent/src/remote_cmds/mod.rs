use nats_utils::{jetstream_client::JsClient, types::JsClientBuilder};

use crate::agent_cli::{self, RemoteArgs, RemoteCommands};

pub(crate) async fn run(args: RemoteArgs, command: RemoteCommands) -> anyhow::Result<()> {
    let nats_client = {
        let nats_url = args.nats_remote_args.nats_url.clone();
        JsClient::new(JsClientBuilder {
            nats_remote_args: args.nats_remote_args,

            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url:?}: {e:?}"))
    }?;

    match command {
        agent_cli::RemoteCommands::Ping {} => {
            let check = nats_client
                .check_connection()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            log::info!("Connection check result: {check}");
        }
    }

    Ok(())
}
