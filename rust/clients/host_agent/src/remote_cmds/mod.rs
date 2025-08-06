pub mod errors;
pub mod types;

use crate::remote_cmds::types::{RemoteArgs, RemoteCommands};
use errors::{RemoteError, RemoteResult};

use db_utils::schemas::workload::{
    Workload, WorkloadManifest, WorkloadState, WorkloadStateDiscriminants, WorkloadStatus,
};
use nats_utils::types::PublishInfo;
use nats_utils::{jetstream_client::JsClient, types::JsClientBuilder};
use workload::types::{WorkloadResult, WorkloadServiceSubjects};

use futures::StreamExt;
use std::str::FromStr;
use std::sync::Arc;

pub(crate) async fn run(args: RemoteArgs, command: RemoteCommands) -> RemoteResult<()> {
    let nats_client = {
        let nats_url = args.nats_remote_args.nats_url.clone();
        JsClient::new(JsClientBuilder {
            nats_remote_args: args.nats_remote_args,
            ..Default::default()
        })
        .await
        .map_err(|e| {
            RemoteError::operation_failed(
                "NATS connection",
                &format!("connecting to NATS via {:?}: {:?}", nats_url, e),
            )
        })
    }?;

    match command {
        RemoteCommands::Ping {} => {
            let check = nats_client
                .check_connection()
                .await
                .map_err(|e| RemoteError::operation_failed("connection check", &e.to_string()))?;

            log::info!("Connection check result: {check}");
        }
        RemoteCommands::HolochainDhtV1Workload {
            workload_id_override,
            host_id,
            desired_status,
            manifest,
            workload_only,
            subject_override,
            maybe_wait_on_subject,
        } => {
            let id: bson::oid::ObjectId = workload_id_override.unwrap_or_default();

            let state_discriminant = WorkloadStateDiscriminants::from_str(&desired_status)
                .map_err(|e| {
                    RemoteError::operation_failed(
                        "parse workload status",
                        &format!("failed to parse '{}': {}", desired_status, e),
                    )
                })?;

            let status = WorkloadStatus {
                id: Some(id),
                desired: WorkloadState::from_repr(state_discriminant as usize).ok_or_else(
                    || {
                        RemoteError::operation_failed(
                            "workload state parsing",
                            &format!("failed to parse workload state '{}'", desired_status),
                        )
                    },
                )?,
                actual: WorkloadState::Unknown("most uncertain".to_string()),
                payload: Default::default(),
            };

            let workload = Workload {
                _id: id,
                status,
                manifest: WorkloadManifest::HolochainDhtV1(manifest),
                metadata: Default::default(),
                assigned_developer: Default::default(),
                version: Default::default(),
                min_hosts: 1,
                assigned_hosts: Default::default(),
                ..Default::default()
            };

            let payload = if workload_only {
                serde_json::to_string_pretty(&workload)
            } else {
                serde_json::to_string_pretty(&WorkloadResult::Workload(workload))
            }?;

            let subject = if let Some(subject) = subject_override {
                subject
            } else if let Some(host_id) = host_id {
                use WorkloadStateDiscriminants::*;
                format!(
                    "WORKLOAD.{host_id}.{}",
                    match state_discriminant {
                        Running => WorkloadServiceSubjects::Command,
                        Uninstalled | Deleted => WorkloadServiceSubjects::Command,
                        Updated => WorkloadServiceSubjects::Command,
                        Reported => WorkloadServiceSubjects::Command,
                        unsupported =>
                            return Err(RemoteError::operation_failed(
                                "workload routing",
                                &format!("don't know where to send {:?}", unsupported)
                            )),
                    }
                )
            } else {
                "WORKLOAD".to_owned()
            };

            // subscribe before sending the message so we see what we send as well.
            if let Some(wait_on_subject) = maybe_wait_on_subject {
                if !args.dont_wait {
                    let mut subscriber = nats_client.subscribe(wait_on_subject.clone()).await?;
                    tokio::spawn(async move {
                        while let Some(msg) = subscriber.next().await {
                            log::info!("[{wait_on_subject}] received message: {msg:#?}");
                        }
                    });
                }
            }

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

            if !args.dont_wait {
                // Only exit program when explicitly requested
                log::info!("waiting until ctrl+c is pressed.");
                tokio::signal::ctrl_c().await.map_err(|e| {
                    RemoteError::operation_failed("signal handling", &e.to_string())
                })?;
            }
        }

        // this immitates what the public holo-gateway is going to do
        // 1. start subscribing on a subject that's used to receive async replies subsequently
        // 2. send a message to the host-agent to request data from the local hc-http-gw instance
        // 3. wait for the first message on the reply subject
        RemoteCommands::HcHttpGwReq { request } => {
            let response = nats_utils::types::hc_http_gw_nats_request(
                Arc::new(nats_client),
                request,
                Default::default(),
            )
            .await?;

            println!("{response:?}");
        }
    }

    Ok(())
}
