use bson::Document;
use anyhow::Result;
use hpos_hal::inventory::HoloInventory;
use mongodb::options::IndexOptions;

use crate::mongodb::traits::IntoIndexes;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct Inventory {
    pub _id: Option<bson::oid::ObjectId>,
    pub metadata: super::metadata::Metadata,

    pub owner: bson::oid::ObjectId,
    pub host: bson::oid::ObjectId,
    /// Hardware inventory information
    pub inventory: HoloInventory,
}

impl IntoIndexes for Inventory {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // Create an index on the owner field
        let owner_index = bson::doc! {
            "owner": 1,
        };
        let owner_index_options = IndexOptions::builder()
            .name("owner_index".to_string())
            .build();
        indices.push((owner_index, Some(owner_index_options)));

        // Create an index on the host field
        let host_index = bson::doc! {
            "host": 1,
        };
        let host_index_options = IndexOptions::builder()
            .name("host_index".to_string())
            .build();
        indices.push((host_index, Some(host_index_options)));

        Ok(indices)
    }
}
