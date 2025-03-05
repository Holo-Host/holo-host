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
    InventoryServiceApi, INVENTORY_SRV_DESC, INVENTORY_SRV_NAME, INVENTORY_SRV_SUBJ,
    INVENTORY_SRV_VERSION, INVENTORY_UPDATE_SUBJECT,
};
use mongodb::Client as MongoDBClient;
use nats_utils::{
    jetstream_client::JsClient,
    types::{ConsumerBuilder, EndpointType, JsServiceBuilder},
};
use std::sync::Arc;

pub async fn run(
    mut nats_client: JsClient,
    db_client: MongoDBClient,
) -> Result<(), async_nats::Error> {
    // Setup JS Stream Service
    let inventory_stream_service = JsServiceBuilder {
        name: INVENTORY_SRV_NAME.to_string(),
        description: INVENTORY_SRV_DESC.to_string(),
        version: INVENTORY_SRV_VERSION.to_string(),
        service_subject: INVENTORY_SRV_SUBJ.to_string(),
    };
    nats_client.add_js_service(inventory_stream_service).await?;

    let inventory_service = nats_client
        .get_js_service(INVENTORY_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to start service. Unable to fetch inventory service."
        ))?;

    // Instantiate the Workload API (requires access to db client)
    let inventory_api = InventoryServiceApi::new(&db_client).await?;

    // Subjects published by hosting agent:
    inventory_service
        .add_consumer(ConsumerBuilder {
            name: "update_host_inventory".to_string(),
            subject: format!("*.{INVENTORY_UPDATE_SUBJECT}"),
            handler: EndpointType::Async(inventory_api.call(
                |api: InventoryServiceApi, msg: Arc<Message>| async move {
                    api.handle_host_inventory_update(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    Ok(())
}
