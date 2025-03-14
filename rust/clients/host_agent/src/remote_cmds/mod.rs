use std::str::FromStr;

use anyhow::Context;
use async_nats::{ConnectOptions, ServerAddr};
use db_utils::schemas::{
    Workload, WorkloadDeployable, WorkloadState, WorkloadStateDiscriminants, WorkloadStatus,
};
use futures::StreamExt;
use url::Url;
use workload::types::WorkloadResult;

use crate::agent_cli::{self, RemoteCommands};

pub(crate) async fn run(nats_url: Url, command: RemoteCommands) -> anyhow::Result<()> {
    log::info!("Trying to connect to {nats_url}...");

    let vanilla_nats_client = async_nats::connect_with_options(
        nats_url.to_string().parse::<ServerAddr>()?,
        ConnectOptions::new().retry_on_initial_connect(),
    )
    .await?;

    match command {
        agent_cli::RemoteCommands::Ping {} => {
            let check = vanilla_nats_client.connection_state().clone();

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
            // run the NATS workload service

            let id: bson::oid::ObjectId = workload_id_override.unwrap_or_default();
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
                min_hosts: Default::default(),
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

            let subject_suffix = {
                use WorkloadStateDiscriminants::*;

                match state_discriminant {
                    Installed | Running => "update",
                    Uninstalled | Deleted | Removed => "update",
                    Updated => "update",
                    unsupported => anyhow::bail!("don't knwo where to send {unsupported:?}"),
                }
            };

            let subject =
                subject_override.unwrap_or_else(|| format!("WORKLOAD.{host_id}.{subject_suffix}"));

            log::debug!("publishing to {subject}:\n{payload:?}");

            vanilla_nats_client
                // .publish_with_reply(subject, reply_subject, payload.into())
                .publish(subject, payload.into())
                .await?;
            vanilla_nats_client.flush().await?;

            // Only exit program when explicitly requested
            log::info!("waiting until ctrl+c is pressed.");
            tokio::signal::ctrl_c().await?;
        }
    }

    Ok(())
}
