use async_nats::ServerAddr;
use db_utils::schemas::{Workload, WorkloadDeployable, WorkloadState, WorkloadStatus};
use futures::StreamExt;
use url::Url;
use workload::types::WorkloadResult;

use crate::{
    agent_cli::{self, RemoteCommands},
    AgentCliError,
};

pub(crate) async fn run(nats_url: Url, command: RemoteCommands) -> anyhow::Result<()> {
    log::info!("Trying to connect to {nats_url}...");

    let vanilla_nats_client =
        async_nats::connect([nats_url.to_string().parse::<ServerAddr>()?].as_slice()).await?;

    match command {
        agent_cli::RemoteCommands::Ping {} => {
            let check = vanilla_nats_client.connection_state().clone();

            log::info!("Connection check result: {check}");
        }
        agent_cli::RemoteCommands::HolochainDhtV1Workload {
            host_id,
            operation,
            deployable,
        } => {
            // run the NATS workload service

            let id: bson::oid::ObjectId = Default::default();
            let reply_subject = format!("REMOTE_CMD.{}", id.to_hex());

            let mut subscription = vanilla_nats_client
                .subscribe(reply_subject.clone())
                .await
                .expect("subscribe works");

            tokio::spawn(async move {
                while let Some(message) = subscription.next().await {
                    println!("{message:#?}");
                }
            });

            let workload = WorkloadResult {
                status: WorkloadStatus {
                    id: Some(id),
                    desired: WorkloadState::Unknown("".to_owned()),
                    actual: WorkloadState::Unknown("".to_owned()),
                },
                workload: Some(Workload {
                    status: WorkloadStatus {
                        id: Some(id),
                        desired: match operation.as_str() {
                            "install" => WorkloadState::Running,
                            "uninstall" => WorkloadState::Uninstalled,
                            other => {
                                anyhow::bail!(AgentCliError::InvalidArguments(format!(
                                    "unknown operation: {other}"
                                )))
                            }
                        },
                        actual: WorkloadState::Unknown("most uncertain".to_string()),
                    },
                    deployable: WorkloadDeployable::HolochainDhtV1(deployable),

                    ..Default::default()
                }),
            };

            vanilla_nats_client
                .publish_with_reply(
                    format!("WORKLOAD.{host_id}.{operation}"),
                    reply_subject,
                    serde_json::to_string_pretty(&workload)
                        .expect("deserialize works")
                        .into(),
                )
                .await?;

            vanilla_nats_client.flush().await?;

            // Only exit program when explicitly requested
            log::info!("waiting until ctrl+c is pressed.");
            tokio::signal::ctrl_c().await?;
        }
    }

    Ok(())
}
