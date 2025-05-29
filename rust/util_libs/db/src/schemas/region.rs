use bson::Document;
use mongodb::options::IndexOptions;

use crate::mongodb::traits::IntoIndexes;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Region {
    pub _id: Option<bson::oid::ObjectId>,
    pub metadata: super::metadata::Metadata,
    /// The code of the region, which is unique and used to identify it.
    pub code: String,
    /// The name of the region, which is a human-readable identifier.
    pub name: String,
}

impl IntoIndexes for Region {
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
