/*
Service Name: ADMIN
Subject: "INVENTORY.>"
Provisioning Account: ADMIN Account (ie: This service is exclusively permissioned to the ADMIN account.)

Users: admin user & host user (the authenticated host user) & auth guard user (the unauthenticated host user)
(NB: Orchestrator admin user can listen to ALL "Inventory.>" subjects)

Endpoints & Managed Subjects:
    - handle_inventory_update: INVENTORY.{{host_pubkey}}
*/

pub mod types;

#[cfg(test)]
mod tests;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, Bson, DateTime};
use db_utils::{
    mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
    schemas::{self, Host, Workload},
};
use hpos_hal::inventory::HoloInventory;
use mongodb::results::UpdateResult;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, sync::Arc};
use types::InventoryApiResult;

pub const INVENTORY_SRV_NAME: &str = "INVENTORY";
pub const INVENTORY_SRV_SUBJ: &str = "INVENTORY";
pub const INVENTORY_SRV_VERSION: &str = "0.0.1";
pub const INVENTORY_SRV_DESC: &str = "This service handles the Inventory updates from Host.";

// Service Endpoint Names:
pub const INVENTORY_UPDATE_SUBJECT: &str = "update";

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
        let host_inventory = Self::convert_msg_to_type::<HoloInventory>(msg)?;
        log::trace!(
            "INVENTORY message payload. subject='{msg_subject}', payload={host_inventory:?}"
        );

        let subject_sections: Vec<&str> = msg_subject.split(".").collect();
        let host_pubkey_index = 1;
        let host_pubkey: schemas::PubKey = subject_sections[host_pubkey_index].into();

        log::debug!("Incoming message for 'INVENTORY.{{host_pubkey}}.update'");
        self.update_host_inventory(&host_pubkey, &host_inventory)
            .await?;

        // Fetch Host collection
        let host = self
            .host_collection
            .get_one_from(doc! { "device_id": &host_pubkey })
            .await?
            .ok_or_else(|| {
                ServiceError::Internal(format!("Failed to fetch Host. host_pubkey={}", host_pubkey))
            })?;

        self.handle_ineligible_host_workloads(host).await?;

        Ok(InventoryApiResult {
            status: types::InventoryUpdateStatus::Ok,
            maybe_response_tags: None,
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

    fn calculate_host_drive_capacity(&self, host_inventory: &HoloInventory) -> i64 {
        host_inventory.drives.iter().fold(0_i64, |mut acc, d| {
            if let Some(capacity) = d.capacity_bytes {
                acc += capacity as i64;
            }
            acc
        })
    }

    async fn handle_ineligible_host_workloads(&self, host: Host) -> Result<(), ServiceError> {
        let host_id = host._id.ok_or_else(|| {
            ServiceError::Internal(format!(
                "Host is missing '_id' field. host_pubkey={}",
                host.device_id
            ))
        })?;

        // Fetch all assigned workloads that exceed the host's capcity in a single query
        let ineligible_assigned_workloads: Vec<ObjectId> = self
            .workload_collection
            .get_many_from(doc! {
                "_id": { "$in": &host.assigned_workloads },
                "$expr": {
                    "$and": [
                        { "$gt": ["$system_specs.capacity.drive", Bson::Int64(self.calculate_host_drive_capacity(&host.inventory))] },
                        { "$gt": ["$system_specs.capacity.cores", Bson::Int64( host.inventory.cpus.len() as i64)] }
                    ]
                }
            })
            .await?
            .into_iter()
            .map(|workload| {
                workload._id.ok_or_else(|| {
                    ServiceError::Internal(format!(
                        "Host is missing '_id' field. host_pubkey={}",
                        host.device_id
                    ))
                })
            })
            .collect::<Result<Vec<ObjectId>, _>>()?;

        // Update database only if there are ineligible workloads
        if !ineligible_assigned_workloads.is_empty() {
            // Remove ineligible workloads from host
            self.host_collection
                .update_one_within(
                    doc! { "_id": host_id },
                    UpdateModifications::Document(doc! {
                        "$pull": { "assigned_workloads": { "$in": &ineligible_assigned_workloads } }
                    }),
                )
                .await?;

            // Remove host from ineligible workloads
            self.workload_collection
                .update_many_within(
                    doc! { "_id": { "$in": &ineligible_assigned_workloads } },
                    UpdateModifications::Document(doc! {
                        "$pull": { "assigned_hosts": host_id }
                    }),
                )
                .await?;
        }

        Ok(())
    }

    // Helper function to initialize mongodb collections
    async fn init_collection<T>(
        client: &MongoDBClient,
        collection_name: &str,
    ) -> Result<MongoCollection<T>>
    where
        T: Serialize
            + for<'de> Deserialize<'de>
            + Unpin
            + Send
            + Sync
            + Default
            + IntoIndexes
            + Debug,
    {
        Ok(MongoCollection::<T>::new(client, schemas::DATABASE_NAME, collection_name).await?)
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!(
                "Error: Failed to deserialize payload. Subject='{}' Err={e:?}",
                msg.subject.clone().into_string(),
            );
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{err_msg:?} Code={:?}", ErrorCode::BAD_REQUEST))
        })
    }
}
