use bson::uuid;
use serde::{Deserialize, Serialize};
use crate::providers::database::schemas::shared::meta::meta_validator;

use super::shared::meta::Meta;

pub const API_KEY_COLLECTION_NAME: &str = "api_keys";

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKey {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub oid: Option<bson::oid::ObjectId>,
    #[serde(rename = "_meta")]
    pub meta: Meta,

    pub user_id: bson::oid::ObjectId,
    pub key: String,
}

pub fn api_key_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["_meta", "user_id", "key"],
        "properties": {
            "_meta": meta_validator(),
            "user_id": {
                "bsonType": "objectId"
            },
            "key": {
                "bsonType": "string"
            }
        }
    }
}
pub fn generate_api_key() -> String {
    // generate 4 parts of the key
    let part1 = uuid::Uuid::new().to_string();
    let part2 = uuid::Uuid::new().to_string();
    let key = format!("{}-{}", part1, part2);
    key
}

pub async fn setup_api_key_indexes(database: &mongodb::Database) -> Result<(), anyhow::Error> {
    let collection = database.collection::<ApiKey>(API_KEY_COLLECTION_NAME);

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "key": 1 })
            .build(),
        None
    ).await?;

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "user_id": 1 })
            .build(),
        None
    ).await?;

    Ok(())
}