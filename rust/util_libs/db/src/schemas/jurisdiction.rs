use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use crate::mongodb::traits::IntoIndexes;

use super::metadata::Metadata;

pub const JURISDICTION_COLLECTION_NAME: &str = "jurisdiction";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Jurisdiction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata:  Metadata,
    /// The code of the jurisdiction, which is unique and used to identify it.
    pub code: String,
    /// The name of the jurisdiction, which is a human-readable identifier.
    pub name: String,
}

impl IntoIndexes for Jurisdiction {
    fn into_indices(self) -> anyhow::Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // Create an index on the code field
        let code_index = bson::doc! {
            "code": 1,
        };
        let code_index_options = IndexOptions::builder()
            .name("code_index".to_string())
            .build();
        indices.push((code_index, Some(code_index_options)));

        Ok(indices)
    }
}
