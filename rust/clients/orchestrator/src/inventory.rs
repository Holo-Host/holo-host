/*
This client is associated with the:
    - ADMIN account
    - admin user

This client is responsible for:
    - registering with the host worklload service to:
        - handling requests to update inventory in DB (for authorized users)
        - handling requests to add inventory stats to error collection for hosts in error state (for unauthorized users)
    - interfacing with mongodb DB
*/

use anyhow::{anyhow, Result};
use async_nats::Message;
use inventory::{
    types::InventoryApiResult, InventoryServiceApi, INVENTORY_SRV_DESC, INVENTORY_SRV_NAME,
    INVENTORY_SRV_SUBJ, INVENTORY_SRV_VERSION,
};
use mongodb::Client as MongoDBClient;
use std::sync::Arc;
use util_libs::nats_js_client::{EndpointType, JsClient};

pub async fn run(
    nats_client: JsClient,
    db_client: MongoDBClient,
) -> Result<nats_client: JsClient, async_nats::Error> {
    // ==================== Setup API & Register Endpoints ====================
    // Setup JS Stream Service
    let service = JsStreamService::new(
        jetstream::new(nats_client.clone()),
        &INVENTORY_SRV_NAME.to_string(),
        &INVENTORY_SRV_DESC.to_string(),
        &INVENTORY_SRV_VERSION.to_string(),
        &INVENTORY_SRV_SUBJ.to_string(),
    );
    let inventory_stream_service = JsService::new(service);
    let orchestrator_workload_client = nats_client
        .add_js_services(vec![inventory_stream_service])
        .await;

    let inventory_stream_service = JsService::new();
    nats_client.add_service(inventory_stream_service);

    // Instantiate the Workload API (requires access to db client)
    let inventory_api = InventoryServiceApi::new(&db_client).await?;

    let inventory_service = nats_client
        .get_js_service(INVENTORY_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Inventory Service. Unable to spin up Orchestrator Inventory Client."
        ))?;

    inventory_service
        .add_consumer::<InventoryApiResult>(
            "update_host_inventory", // consumer name
            &INVENTORY_SRV_NAME,     // consumer stream subj
            EndpointType::Async(inventory_api.call(
                |api: InventoryServiceApi, msg: Arc<Message>| async move {
                    api.handle_host_inventory_update(msg).await
                },
            )),
            None,
        )
        .await?;

    Ok(nats_client)
}
