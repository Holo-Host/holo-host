/*
Service Name: ADMIN
Subject: "INVENTORY.>"
Provisioning Account: ADMIN Account (ie: This service is exclusively permissioned to the ADMIN account.)
Users: admin user & host_<host_pubkey> user (the authenticated host user) & auth guard user (the unauthenticated host user)
Endpoints & Managed Subjects:
    - handle_inventory_update: INVENTORY.host_<host_pubkey>.authenticated
    - handle_error_host_inventory: INVENTORY.host_<host_pubkey>.unauthenticated
*/

pub mod types;
use anyhow::{Context, Result};
use async_nats::jetstream::ErrorCode;
use async_nats::HeaderValue;
use async_nats::{Message};
use core::option::Option::None;
use mongodb::Client as MongoDBClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use types::InventoryApiResult;
use util_libs::{
    db::{
        mongodb::{IntoIndexes, MongoCollection}, // MongoDbAPI
        schemas::{self, Host, Hoster, User},     // , RoleInfo
    },
    nats_js_client::{get_nats_jwt_by_nsc, AsyncEndpointHandler, JsServiceResponse, ServiceError},
};

pub const INVENTORY_SRV_NAME: &str = "INVENTORY";
pub const INVENTORY_SRV_SUBJ: &str = "INVENTORY";
pub const INVENTORY_SRV_VERSION: &str = "0.0.1";
pub const INVENTORY_SRV_DESC: &str =
    "This service handles the Inventory updates from Host.";

// Service Endpoint Names:
pub const INVENTORY_UPDATE_SUBJECT: &str = "authenticated";
pub const HOST_DEVICE_ERROR_STATE_SUBJECT: &str = "unauthenticated";

#[derive(Clone, Debug)]
pub struct InventoryServiceApi {
    pub developer_collection: MongoCollection<Developer>,
    pub host_collection: MongoCollection<Host>,
}

impl InventoryServiceApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            developer_collection: Self::init_collection(client, schemas::DEVELOPER_COLLECTION_NAME)
                .await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    pub async fn handle_host_inventory_update(
        &self,
        msg: Arc<Message>,
    ) {
        log::debug!("Incoming message for 'INVENTORY.update'");
        let message_payload = Self::convert_msg_to_type::<InventoryPayloadType>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        match InventoryPayloadType {
            authenticated => {
                log::debug!("Incoming message for 'INVENTORY.update.host_<host_pk>.authenticated'");

                let unqualified_assigned_hosts: Vec<&schemas::MongoDbId>;
                // Check for to ensure assigned host *still* has enough capacity for updated workload
                for host_id in workload.assigned_hosts.iter() {
                    let host = self.host_collection.get_one_from( doc! { "_id": host_id }).await?.ok_or(|e: anyhow::Error| ServiceError::Internal(e.to_string()))?;
                    if self.verify_host_meets_workload_criteria(workload, host) == false {
                        unqualified_hosts.push(host_id);
                    };
                }
                if !unqualified_hosts.is_empty() {
                }

                // ...
            },
            unauthenticated => {
                log::debug!("Incoming message for 'INVENTORY.update.host_<host_pk>.unauthenticated'");

                // ...
            }
        }
    }

    // Helper function to initialize mongodb collections
    async fn init_collection<T>(
        client: &MongoDBClient,
        collection_name: &str,
    ) -> Result<MongoCollection<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
    {
        Ok(MongoCollection::<T>::new(client, schemas::DATABASE_NAME, collection_name).await?)
    }

    pub fn call<F, Fut>(&self, handler: F) -> AsyncEndpointHandler<AuthApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AuthApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync,
    {
        let api = self.to_owned();
        Arc::new(
            move |msg: Arc<Message>| -> JsServiceResponse<AuthApiResult> {
                let api_clone = api.clone();
                Box::pin(handler(api_clone, msg))
            },
        )
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!(
                "Error: Failed to deserialize payload. Subject='{}' Err={}",
                msg.subject.clone().into_string(),
                e
            );
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
    }

    // Verifies that a host meets the workload criteria
    pub fn verify_host_meets_workload_criteria(
        &self,
        workload: Workload,
        assigned_host: Host,
    ) -> bool {
        if assigned_host.remaining_capacity.disk < workload.system_specs.capacity.disk {
            return false;
        }
        if assigned_host.remaining_capacity.memory < workload.system_specs.capacity.memory {
            return false;
        }
        if assigned_host.remaining_capacity.cores < workload.system_specs.capacity.cores {
            return false;
        }

        true
    }
}
