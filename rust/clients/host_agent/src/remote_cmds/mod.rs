use anyhow::Context;
use async_nats::HeaderName;
use db_utils::schemas::{
    Workload, WorkloadManifest, WorkloadState, WorkloadStateDiscriminants, WorkloadStatus,
};
use futures::StreamExt;
use nats_utils::types::{HcHttpGwResponse, PublishInfo};
use nats_utils::{jetstream_client::JsClient, types::JsClientBuilder};
use std::str::FromStr;
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
            manifest,
            workload_only,
            subject_override,
            maybe_wait_on_subject,
        } => {
            let id: bson::oid::ObjectId = workload_id_override.unwrap_or_default();

            let state_discriminant = WorkloadStateDiscriminants::from_str(&desired_status)
                .map_err(|e| anyhow::anyhow!("failed to parse {desired_status}: {e}"))?;

            let status = WorkloadStatus {
                id: Some(id),
                desired: WorkloadState::from_repr(state_discriminant as usize)
                    .ok_or_else(|| anyhow::anyhow!("failed to parse {desired_status}"))?,
                actual: WorkloadState::Unknown("most uncertain".to_string()),
                payload: Default::default(),
            };

            let workload = Workload {
                _id: Some(id),
                status,
                manifest: WorkloadManifest::HolochainDhtV1(manifest),

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
            } else if let Some(host_id) = host_id {
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
                tokio::signal::ctrl_c().await?;
            }
        }

        // this immitates what the public holo-gateway is going to do
        // 1. start subscribing on a subject that's used to receive async replies subsequently
        // 2. send a message to the host-agent to request data from the local hc-http-gw instance
        // 3. wait for the first message on the reply subject
        agent_cli::RemoteCommands::HcHttpGwReq { request } => {
            let destination_subject = request.nats_destination_subject();
            let reply_subject = request.nats_reply_subject();

            let response = {
                let data = serde_json::to_string(&request)?;

                let _ack = nats_client
                    .js_context
                    .publish_with_headers(
                        destination_subject.clone(),
                        async_nats::HeaderMap::from_iter([(
                            HeaderName::from_static(nats_utils::jetstream_service::JsStreamService::HEADER_NAME_REPLY_OVERRIDE),
                            async_nats::HeaderValue::from_str(&reply_subject).unwrap(),
                        )]),
                        data.into(),
                    )
                    .await?;
                log::info!("request published");

                let mut response = nats_client.client.subscribe(reply_subject.clone()).await?;

                let msg = response
                    .next()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("got no response on subject {reply_subject}"))?;

                let response: HcHttpGwResponse = serde_json::from_slice(&msg.payload)?;
                response
            };

            println!("{response:?}");
        }
    }

    Ok(())
}
