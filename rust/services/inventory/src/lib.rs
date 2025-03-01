/*
Service Name: ADMIN
Subject: "INVENTORY.>"
Provisioning Account: ADMIN Account (ie: This service is exclusively permissioned to the ADMIN account.)

Users: admin user & host user (the authenticated host user) & auth guard user (the unauthenticated host user)
(NB: Orchestrator admin user can listen to ALL "Inventory.>" subjects)

Endpoints & Managed Subjects:
    - handle_inventory_update: INVENTORY.{{host_pubkey}}.authenticated
    - handle_error_host_inventory: INVENTORY.{{host_pubkey}}.unauthenticated
*/

pub mod types;

#[cfg(test)]
mod tests;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use bson::oid::ObjectId;
use bson::{self, doc, DateTime};
use hpos_hal::inventory::HoloInventory;
use mongodb::results::UpdateResult;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use types::{InventoryApiResult, InventoryPayloadType};
use util_libs::db::mongodb::MongoDbAPI;
use util_libs::{
    db::{
        mongodb::{IntoIndexes, MongoCollection},
        schemas::{self, Host, Workload},
    },
    nats::types::{AsyncEndpointHandler, JsServiceResponse, ServiceError},
};

pub const INVENTORY_SRV_NAME: &str = "INVENTORY";
pub const INVENTORY_SRV_SUBJ: &str = "INVENTORY";
pub const INVENTORY_SRV_VERSION: &str = "0.0.1";
pub const INVENTORY_SRV_DESC: &str = "This service handles the Inventory updates from Host.";

// Service Endpoint Names:
pub const HOST_UNAUTHENTICATED_SUBJECT: &str = "unauthenticated";
pub const HOST_AUTHENTICATED_SUBJECT: &str = "authenticated";
pub const INVENTORY_UPDATE_SUBJECT: &str = "*.update";

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
        let msg_subject = msg.subject.clone().into_string();
        let message_payload = Self::convert_msg_to_type::<InventoryPayloadType>(msg)?;
        log::trace!(
            "INVENTORY message payload. subject='{}', msg={:?}",
            msg_subject,
            message_payload
        );

        let subject_sections: Vec<&str> = msg_subject.split(".").collect();
        let host_pubkey: schemas::PubKey = subject_sections[2].into();

        match message_payload {
            InventoryPayloadType::Authenticated(host_inventory) => {
                log::debug!(
                    "Incoming message for 'INVENTORY.authenticated.{{host_pubkey}}.update'"
                );
                self.update_host_inventory(&host_pubkey, &host_inventory)
                    .await?;

                // Fetch Host collection
                let host = self
                    .host_collection
                    .get_one_from(doc! { "device_id": &host_pubkey })
                    .await?
                    .ok_or(ServiceError::Internal(format!(
                        "Failed to fetch Host. host_pubkey={}",
                        host_pubkey
                    )))?;

                let host_id = host._id.ok_or(ServiceError::Internal(format!(
                    "Failed to fetch Host. host_pubkey={}",
                    host_pubkey
                )))?;

                // Ensure assigned host *still* has enough capacity for assigned workload(s)
                // ..and if no, remove host from workload and create collection of all ineligible workloads
                let mut ineligible_assigned_workloads: Vec<ObjectId> = vec![];
                for workload_id in host.assigned_workloads {
                    let workload = self
                        .workload_collection
                        .get_one_from(doc! { "_id": workload_id })
                        .await?
                        .ok_or(ServiceError::Internal(format!(
                            "Failed to fetch Workload. workload_id={}",
                            workload_id
                        )))?;

                    if !self.verify_host_meets_workload_criteria(&host_inventory, &workload) {
                        ineligible_assigned_workloads.push(workload_id);

                        self.host_collection
                            .update_one_within(
                                doc! { "_id": host_id },
                                UpdateModifications::Document(doc! {
                                    "$pull": {
                                        "assigned_workloads": workload_id
                                    }
                                }),
                            )
                            .await?;
                    };
                }
                // ...and remove host from all ineligible workloads
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
            InventoryPayloadType::Unauthenticated(host_inventory) => {
                log::debug!(
                    "Incoming message for 'INVENTORY.unauthenticated.{{host_pubkey}}.update'"
                );
                self.update_host_inventory(&host_pubkey, &host_inventory)
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

    // Add updated Holo Inventory to Host collection
    async fn update_host_inventory(
        &self,
        host_pubkey: &schemas::PubKey,
        inventory: &HoloInventory,
    ) -> Result<UpdateResult, ServiceError> {
        self.host_collection
            .update_one_within(
                doc! { "device_id": host_pubkey },
                UpdateModifications::Document(doc! {
                    "$set": {
                        "inventory": bson::to_bson(inventory)
                            .map_err(|e| ServiceError::Internal(e.to_string()))?,
                        "metadata.updated_at": DateTime::now()
                    }
                }),
            )
            .await
    }

    // Verifies that a host meets the workload criteria
    pub fn verify_host_meets_workload_criteria(
        &self,
        host_inventory: &HoloInventory,
        workload: &Workload,
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

        true
    }
}
