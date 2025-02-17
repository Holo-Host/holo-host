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
use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use bson::{self, doc, DateTime};
use hpos_hal::inventory::HoloInventory;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use types::{InventoryApiResult, InventoryPayloadType};
use util_libs::db::mongodb::MongoDbAPI;
use util_libs::{
    db::{
        mongodb::{IntoIndexes, MongoCollection},
        schemas::{self, Host, Workload},
    },
    nats_js_client::{AsyncEndpointHandler, JsServiceResponse, ServiceError},
};

pub const INVENTORY_SRV_NAME: &str = "INVENTORY";
pub const INVENTORY_SRV_SUBJ: &str = "INVENTORY";
pub const INVENTORY_SRV_VERSION: &str = "0.0.1";
pub const INVENTORY_SRV_DESC: &str = "This service handles the Inventory updates from Host.";

// Service Endpoint Names:
pub const INVENTORY_UPDATE_SUBJECT: &str = "authenticated";
pub const HOST_DEVICE_ERROR_STATE_SUBJECT: &str = "unauthenticated";

#[derive(Clone, Debug)]
pub struct InventoryServiceApi {
    pub workload_collection: MongoCollection<Workload>,
    pub host_collection: MongoCollection<Host>,
}

impl InventoryServiceApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME)
                .await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    pub async fn handle_host_inventory_update(
        &self,
        msg: Arc<Message>,
    ) -> Result<InventoryApiResult, ServiceError> {
        log::debug!("Incoming message for 'INVENTORY.update'");
        let msg_subject = msg.subject.clone().into_string();
        let message_payload = Self::convert_msg_to_type::<InventoryPayloadType>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let subject_sections: Vec<&str> = msg_subject.split(".").collect();
        let host_pubkey = subject_sections[2];
        let host_id = schemas::MongoDbId::from_str(host_pubkey)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let inventory = match message_payload.clone() {
            InventoryPayloadType::Authenticated(i) => {
                log::debug!("Incoming message for 'INVENTORY.update.<>.authenticated'");
                i
            }
            InventoryPayloadType::Unauthenticated(i) => {
                log::debug!("Incoming message for 'INVENTORY.update.<>.unauthenticated'");
                i
            }
        };

        // Add Update Inventory to Host collection
        self
            .host_collection
            .update_one_within(
                doc! { "_id": host_id },
                UpdateModifications::Document(doc! { "$set": doc! {
                    "$set": {
                        "inventory": bson::to_bson(&inventory).map_err(|e| ServiceError::Internal(e.to_string()))?,
                        "metadata.updated_at": DateTime::now()
                    }
                }}),
            )
            .await?;

        if let InventoryPayloadType::Authenticated(host_inventory) = message_payload {
            // Fetch Host collection
            let host = self
                .host_collection
                .get_one_from(doc! { "_id": host_id })
                .await?
                .ok_or(ServiceError::Internal(format!(
                    "Failed to fetch Host. host_id={}",
                    host_id
                )))?;

            // Check for to ensure assigned host *still* has enough capacity for assigned workload(s)
            let mut ineligible_assigned_workloads: Vec<schemas::MongoDbId> = vec![];
            for workload_id in host.assigned_workloads {
                let workload = self
                    .workload_collection
                    .get_one_from(doc! { "_id": workload_id })
                    .await?
                    .ok_or(ServiceError::Internal(format!(
                        "Failed to fetch Workload. workload_id={}",
                        host_id
                    )))?;

                if !self.verify_host_meets_workload_criteria(&workload, &host_inventory) {
                    ineligible_assigned_workloads.push(workload_id);
                };
            }
            if !ineligible_assigned_workloads.is_empty() {
                self.workload_collection
                    .update_many_within(
                        doc! { "_id": { "$in": ineligible_assigned_workloads } },
                        UpdateModifications::Document(doc! {
                            "$pull": {
                                "assigned_hosts": host_id
                            }
                        }),
                    )
                    .await?;
            }
        }

        Ok(InventoryApiResult {
            status: types::InventoryUpdateStatus::Ok,
            maybe_response_tags: None,
        })
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

    pub fn call<F, Fut>(&self, handler: F) -> AsyncEndpointHandler<InventoryApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<InventoryApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync,
    {
        let api = self.to_owned();
        Arc::new(
            move |msg: Arc<Message>| -> JsServiceResponse<InventoryApiResult> {
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
        workload: &Workload,
        host_inventory: &HoloInventory,
    ) -> bool {
        let host_drive_capacity = host_inventory.drives.iter().fold(0, |mut acc, d| {
            if let Some(capacity) = d.capacity_bytes {
                acc += capacity;
            }
            acc
        });
        if host_drive_capacity < workload.system_specs.capacity.drive {
            return false;
        }
        if host_inventory.cpus.len() < workload.system_specs.capacity.cores as usize {
            return false;
        }
        // if host_inventory.memory < workload.system_specs.capacity.memory {
        //     return false;
        // }

        true
    }
}
