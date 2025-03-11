use async_nats::ServerAddr;
use db_utils::schemas::{Workload, WorkloadState, WorkloadStatus};
use url::Url;
use workload::types::WorkloadResult;

use crate::{
    agent_cli::{self, RemoteCommands},
    AgentCliError,
};

pub(crate) async fn run(nats_url: Url, command: RemoteCommands) -> Result<(), AgentCliError> {
    log::info!("Trying to connect to {nats_url}...");

    let vanilla_nats_client =
        async_nats::connect([nats_url.to_string().parse::<ServerAddr>()?].as_slice())
            .await
            .map_err(|e| AgentCliError::AsyncNats(Box::new(e)))?;

    match command {
        agent_cli::RemoteCommands::Ping {} => {
            let check = vanilla_nats_client.connection_state().clone();

            log::info!("Connection check result: {check}");
        }
        agent_cli::RemoteCommands::Workload { operation, data } => {
            let id: bson::oid::ObjectId = Default::default();
            let reply_subject = format!("REMOTE_CMD.{}", id.to_hex());

            let workload = WorkloadResult {
                status: WorkloadStatus {
                    id: Some(id),
                    desired: WorkloadState::Unknown("".to_owned()),
                    actual: WorkloadState::Unknown("".to_owned()),
                },
                workload: Some(Workload {
                    nix_pkg: data,

                    status: WorkloadStatus {
                        id: Some(id),
                        desired: match operation.as_str() {
                            "install" => WorkloadState::Running,
                            "uninstall" => WorkloadState::Uninstalled,
                            other => {
                                return Err(AgentCliError::InvalidArguments(format!(
                                    "unknown operation: {other}"
                                )))
                            }
                        },
                        actual: WorkloadState::Unknown("most uncertain".to_string()),
                    },

                    ..Default::default()
                }),
            };

            vanilla_nats_client
                .publish_with_reply(
                    format!("WORKLOAD.host_pubkey_placeholder.{operation}"),
                    reply_subject,
                    serde_json::to_string_pretty(&workload)
                        .expect("deserialize works")
                        .into(),
                )
                .await
                .map_err(|e| AgentCliError::AsyncNats(Box::new(e)))?;

            vanilla_nats_client
                .flush()
                .await
                .map_err(|e| AgentCliError::AsyncNats(Box::new(e)))?;
        }
    }

    Ok(())
}
