use std::str::FromStr;

use anyhow::Context;
use db_utils::schemas::{
    Workload, WorkloadDeployable, WorkloadState, WorkloadStateDiscriminants, WorkloadStatus,
};
use nats_utils::{
    jetstream_client::JsClient,
    types::{JsClientBuilder, PublishInfo},
};
use workload::types::{WorkloadResult, WorkloadServiceSubjects};

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
        agent_cli::RemoteCommands::HolochainDhtV1Workload {
            workload_id_override,
            host_id,
            desired_status,
            deployable,
            workload_only,
            subject_override,
        } => {
            let id: bson::oid::ObjectId = workload_id_override.unwrap_or_default();

            let state_discriminant = WorkloadStateDiscriminants::from_str(&desired_status)
                .map_err(|e| anyhow::anyhow!("failed to parse {desired_status}: {e}"))?;

            let status = WorkloadStatus {
                id: Some(id),
                desired: WorkloadState::from_repr(state_discriminant as usize)
                    .ok_or_else(|| anyhow::anyhow!("failed to parse {desired_status}"))?,
                actual: WorkloadState::Unknown("most uncertain".to_string()),
            };

            let workload = Workload {
                _id: Some(id),
                status,
                deployable: WorkloadDeployable::HolochainDhtV1(deployable),

                metadata: Default::default(),
                assigned_developer: Default::default(),
                version: Default::default(),
                min_hosts: 1,
                assigned_hosts: Default::default(),

                ..Default::default() // ---
                                     // these don't have defaults on their own
                                     // system_specs: Default::default(),
            };

            let payload = if workload_only {
                serde_json::to_string_pretty(&workload)
            } else {
                serde_json::to_string_pretty(&WorkloadResult {
                    status: workload.status.clone(),
                    workload: Some(workload),
                })
            }
            .context("serializing workload payload")?;

            let subject = if let Some(subject) = subject_override {
                subject
            } else {
                use WorkloadStateDiscriminants::*;

                format!(
                    "WORKLOAD.{host_id}.{}",
                    match state_discriminant {
                        Installed | Running => WorkloadServiceSubjects::Command,
                        Uninstalled | Deleted | Removed => WorkloadServiceSubjects::Command,
                        Updated => WorkloadServiceSubjects::Command,
                        Reported => WorkloadServiceSubjects::Command,
                        unsupported => anyhow::bail!("don't know where to send {unsupported:?}"),
                    }
                )
            };

            log::debug!("publishing to {subject}:\n{payload:?}");

            if let Ok(response) = nats_client
                .publish(PublishInfo {
                    subject,
                    msg_id: Default::default(),
                    data: payload.into(),
                    headers: None,
                })
                .await
            {
                log::info!("request completed. response: {response:#?}");
            };

            // // Only exit program when explicitly requested
            // log::info!("waiting until ctrl+c is pressed.");
            // tokio::signal::ctrl_c().await?;
        }
    }

    Ok(())
}
