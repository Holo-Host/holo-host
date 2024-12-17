/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: AUTH Account
Importing Account: Auth/NoAuth Account

This service should be run on the ORCHESTRATOR side and called from the HPOS side.
The NoAuth/Auth Server will import this service on the hub side and read local jwt files once the agent is validated.
NB: subject pattern = "<SERVICE_NAME>.<Subject>.<DirectObject>.<Verb>.<Details>"
This service handles the the "AUTH.<host_id>.file.transfer.JWT-<hoster_pubkey>.<chunk_id>" subject

Endpoints & Managed Subjects:
    - start_hub_handshake
    - end_hub_handshake
    - save_hub_auth
    - save_user_auth

*/

mod endpoints;
mod types;
use anyhow::Result;
use async_nats::Message;
use bytes::Bytes;
use dotenv::dotenv;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use util_libs::{
    js_microservice::JsStreamService,
    nats_client::{self, EndpointType},
};

const AUTH_SRV_OWNER_NAME: &str = "AUTH_OWNER";
const AUTH_SRV_NAME: &str = "AUTH";
const AUTH_SRV_SUBJ: &str = "AUTH";
const AUTH_SRV_VERSION: &str = "0.0.1";
const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the HPOS and the Orchestrator.";

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // ==================== NATS Setup ====================

    let nats_url = nats_client::get_nats_url();
    let creds_path = nats_client::get_nats_client_creds("HOLO", "ADMIN", "orchestrator");
    let event_listeners = endpoints::get_event_listeners();

    let auth_service_inbox_prefix: &str = "_auth";

    let auth_service = nats_client::DefaultClient::new(nats_client::NewDefaultClientParams {
        nats_url,
        name: AUTH_SRV_OWNER_NAME.to_string(),
        inbox_prefix: auth_service_inbox_prefix.to_string(),
        opts: vec![nats_client::with_event_listeners(event_listeners)],
        credentials_path: Some(creds_path),
        ..Default::default()
    })
    .await?;

    // Create a new Jetstream Microservice
    let js_context = JsStreamService::get_context(auth_service.client.clone());
    let js_service = JsStreamService::new(
        js_context,
        AUTH_SRV_NAME,
        AUTH_SRV_DESC,
        AUTH_SRV_VERSION,
        AUTH_SRV_SUBJ,
    )
    .await?;

    // ==================== API ENDPOINTS ====================

    js_service
        .add_local_consumer(
            "publish_hub_files", // called from hpos
            "start_hub_handshake",
            nats_client::EndpointType::Async(endpoints::start_handshake().await),
            None,
        )
        .await?;

    js_service
        .add_local_consumer(
            "save_hub_auth", // called from hpos
            "save_hub_auth",
            nats_client::EndpointType::Async(endpoints::save_hub_auth().await),
            None,
        )
        .await?;

    js_service
        .add_local_consumer(
            "send_user_pubkey", // called from hpos
            "send_user_pubkey",
            nats_client::EndpointType::Async(endpoints::send_user_pubkey().await),
            None,
        )
        .await?;

    log::trace!(
        "{} Service is running. Waiting for requests...",
        AUTH_SRV_NAME
    );

    Ok(())
}
