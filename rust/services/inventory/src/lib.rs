/*
Service Name: ADMIN
Subject: "INVENTORY.>"
Provisioning Account: ADMIN Account (ie: This service is exclusively permissioned to the ADMIN account.)

Users: admin user & host user (the authenticated host user) & auth guard user (the unauthenticated host user)
(NB: Orchestrator admin user can listen to ALL "Inventory.>" subjects)

Endpoints & Managed Subjects:
    - handle_inventory_update: INVENTORY.{{host_id}}
*/

pub mod types;

#[cfg(test)]
mod tests;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, DateTime};
use db_utils::{
    mongodb::{
        api::MongoDbAPI,
        collection::MongoCollection,
        traits::{IntoIndexes, MutMetadata},
    },
    schemas::{
        self,
        host::{Host, HOST_COLLECTION_NAME},
        workload::{Workload, WORKLOAD_COLLECTION_NAME},
    },
};
use hpos_hal::inventory::HoloInventory;
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
            workload_collection: Self::init_collection(client, WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
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

        let subject_sections: Vec<&str> = msg_subject.split('.').collect();
        let host_device_id_index = 1;
        let host_device_id: schemas::alias::PubKey = subject_sections
            .get(host_device_id_index)
            .ok_or_else(|| {
                ServiceError::internal(
                    "Invalid subject format",
                    Some("Missing host pubkey in subject".to_string()),
                )
            })?
            .to_string();

        log::debug!("Processing inventory update for host: {host_device_id}");

        // Update host inventory and get the host record
        let host = self
            .update_host_inventory(&host_device_id, &host_inventory)
            .await?;

        // Handle workloads that are no longer compatible with the host
        self.handle_ineligible_host_workloads(host).await?;

        Ok(InventoryApiResult {
            status: types::InventoryUpdateStatus::Ok,
            maybe_response_tags: None,
        })
    }

    // Update Host's Holo Inventory in Host collection,
    // creating a new Host entry if one doesn't already exist for the provided host_device_id
    async fn update_host_inventory(
        &self,
        host_device_id: &str,
        inventory: &HoloInventory,
    ) -> Result<Host, ServiceError> {
        // Create a default Host instance to extract default values
        let default_host = Host::default();
        let filter = doc! { "device_id": host_device_id };
        let update = doc! {
            "$set": {
                "inventory": bson::to_bson(inventory)
                    .map_err(|e| ServiceError::internal(e.to_string(), None))?,
                "metadata.updated_at": DateTime::now(),
            },
            // If the document doesn't exist, also set the device_id (host_device_id)
            "$setOnInsert": {
                "metadata.is_deleted": false,
                "metadata.created_at": DateTime::now(),
                "device_id": host_device_id,
                "avg_uptime": default_host.avg_uptime,
                "avg_network_speed": default_host.avg_network_speed,
                "avg_latency": default_host.avg_latency,
                "assigned_workloads": [],
            }
        };

        // Use upsert to either insert or update the document
        let host = self
            .host_collection
            .inner
            .find_one_and_update(filter, UpdateModifications::Document(update))
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .await?
            .ok_or_else(|| {
                ServiceError::internal(
                    "Failed to return Host record after calling `find_one_and_update`.",
                    Some("Host Collection".to_string()),
                )
            })?;

        Ok(host)
    }

    fn calculate_host_drive_capacity(&self, host_inventory: &HoloInventory) -> i64 {
        host_inventory
            .drives
            .iter()
            .fold(0_i64, |acc, d| acc + d.capacity_bytes.unwrap_or(0) as i64)
    }

    async fn handle_ineligible_host_workloads(&self, host: Host) -> Result<(), ServiceError> {
        let host_id = host._id.ok_or_else(|| {
            ServiceError::internal(
                format!("Host missing ID: {}", host.device_id),
                Some("Database integrity error".to_string()),
            )
        })?;

        // Find workloads that exceed host capacity
        let host_drive_capacity = self.calculate_host_drive_capacity(&host.inventory);
        let host_cpu_count = host.inventory.cpus.len() as i64;

        let ineligible_workloads = self
            .workload_collection
            .get_many_from(doc! {
                "_id": { "$in": &host.assigned_workloads },
                "$or": [
                    { "system_specs.capacity.drive": { "$gt": host_drive_capacity } },
                    { "system_specs.capacity.cores": { "$gt": host_cpu_count } }
                ]
            })
            .await?;

        let ineligible_workload_ids: Vec<ObjectId> = ineligible_workloads
            .into_iter()
            .filter_map(|w| w._id)
            .collect();

        if !ineligible_workload_ids.is_empty() {
            log::info!(
                "Removing {} ineligible workloads from host {}",
                ineligible_workload_ids.len(),
                host.device_id
            );

            // Remove ineligible workloads from host
            self.host_collection
                .update_one_within(
                    doc! { "_id": host_id },
                    UpdateModifications::Document(doc! {
                        "$pull": { "assigned_workloads": { "$in": &ineligible_workload_ids } }
                    }),
                    false,
                )
                .await?;

            // Remove host from ineligible workloads
            self.workload_collection
                .update_many_within(
                    doc! { "_id": { "$in": &ineligible_workload_ids } },
                    UpdateModifications::Document(doc! {
                        "$pull": { "assigned_hosts": host_id }
                    }),
                    false,
                )
                .await?;
        }

        Ok(())
    }

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
            + Debug
            + IntoIndexes
            + MutMetadata,
    {
        let db_name = std::env::var("MONGODB_NAME").unwrap_or(schemas::DATABASE_NAME.to_string());
        Ok(MongoCollection::<T>::new(client, &db_name, collection_name).await?)
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        serde_json::from_slice::<T>(&msg.payload).map_err(|e| {
            ServiceError::request(
                format!("Failed to deserialize payload: {}", e),
                Some(ErrorCode::BAD_REQUEST),
            )
        })
    }
}
