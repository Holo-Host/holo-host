/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user

// This client is responsible for:
*/

mod api;
mod endpoints;
use anyhow::Result;
use dotenv::dotenv;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use util_libs::{
    db::{mongodb::get_mongodb_url, schemas},
    js_microservice::JsStreamService,
    nats_client::{self, EventListener},
};

const ORCHESTRATOR_CLIENT_NAME: &str = "Orchestrator Agent";
const ORCHESTRATOR_CLIENT_INBOX_PREFIX: &str = "_orchestrator_inbox";

#[tokio::main]
async fn main() -> Result<(), async_nats::Error> {
    dotenv().ok();
    env_logger::init();

    // ==================== NATS Setup ====================
    let nats_url = nats_client::get_nats_url();
    let creds_path = nats_client::get_nats_client_creds("HOLO", "WORKLOAD", "orchestrator");
    let event_listeners = endpoints::get_orchestrator_workload_event_listeners();

    let workload_service_inbox_prefix: &str = "_workload";

    let workload_service = nats_client::DefaultClient::new(nats_client::NewDefaultClientParams {
        nats_url,
        name: WORKLOAD_SRV_OWNER_NAME.to_string(),
        inbox_prefix: workload_service_inbox_prefix.to_string(),
        opts: vec![nats_client::with_event_listeners(event_listeners)],
        credentials_path: Some(creds_path),
        ..Default::default()
    })
    .await?;

    // Create a new Jetstream Microservice
    let js_context = JsStreamService::get_context(workload_service.client.clone());
    let js_service = JsStreamService::new(
        js_context,
        WORKLOAD_SRV_NAME,
        WORKLOAD_SRV_DESC,
        WORKLOAD_SRV_VERSION,
        WORKLOAD_SRV_SUBJ,
    )
    .await?;

    // ==================== DB Setup ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // // Create a typed collection for User
    // let mut user_api = MongoCollection::<schemas::User>::new(
    //     &client,
    //     schemas::DATABASE_NAME,
    //     schemas::HOST_COLLECTION_NAME,
    // )
    // .await?;

    // // Create a typed collection for Host
    // let mut host_api = MongoCollection::<schemas::Host>::new(
    //     &client,
    //     schemas::DATABASE_NAME,
    //     schemas::HOST_COLLECTION_NAME,
    // )
    // .await?;

    // Create a typed collection for Workload
    let workload_api = api::WorkloadApi::new(&client).await?;

    // ==================== API ENDPOINTS ====================

    // For ORCHESTRATOR to consume
    // (subjects should be published by developer)
    js_service
        .add_local_consumer(
            "add_workload",
            "add",
            nats_client::EndpointType::Async(endpoints::add_workload(workload_api).await),
            None,
        )
        .await?;

    js_service
        .add_local_consumer(
            "handle_changed_db_workload",
            "handle_change",
            nats_client::EndpointType::Async(endpoints::handle_db_change().await),
            None,
        )
        .await?;

 
    log::trace!(
        "{} Service is running. Waiting for requests...",
        WORKLOAD_SRV_NAME
    );

    Ok(())
}

pub fn get_orchestrator_workload_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = |msg: &str, _duration: Duration| {
        log::info!(
            "Successfully published message for {} client: {:?}",
            WORKLOAD_SRV_OWNER_NAME,
            msg
        );
    };
    let failure_handler = |err: &str, _duration: Duration| {
        log::error!(
            "Failed to publish message for {} client: {:?}",
            WORKLOAD_SRV_OWNER_NAME,
            err
        );
    };

    let event_listeners = vec![
        nats_client::on_msg_published_event(published_msg_handler),
        nats_client::on_msg_failed_event(failure_handler),
    ];

    event_listeners
}
