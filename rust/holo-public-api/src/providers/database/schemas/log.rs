use mongodb::Database;
use serde::{Deserialize, Serialize};
use super::shared::{
    meta::meta_validator,
    meta::Meta
};

pub const LOG_COLLECTION_NAME: &str = "logs";
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Log {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<bson::oid::ObjectId>,
    pub _meta: Meta,

    pub id: String,
    pub timestamp: bson::DateTime,
    pub path: String,
    pub method: String,
    pub ip: String,
    pub user_agent: String,
    pub authorization: String,
    pub user_id: String,
    pub status: i32,
}

pub fn log_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": [
            "_meta",
            "timestamp",
            "path",
            "method",
            "ip",
            "user_agent",
            "authorization",
            "user_id",
            "status"
        ],
        "properties": {
            "_meta": meta_validator(),
            "id": {
                "bsonType": "string"
            },
            "timestamp": {
                "bsonType": "date"
            },
            "path": {
                "bsonType": "string"
            },
            "method": {
                "bsonType": "string"
            },
            "ip": {
                "bsonType": "string"
            },
            "user_agent": {
                "bsonType": "string"
            },
            "authorization": {
                "bsonType": "string"
            },
            "user_id": {
                "bsonType": "string"
            },
            "status": {
                "bsonType": "int"
            }
        }
    }
}

pub async fn setup_log_indexes(database: &Database) -> Result<(), anyhow::Error> {
    let collection = database.collection::<Log>(LOG_COLLECTION_NAME);

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "user_id": 1, "timestamp": 1 })
            .build(),
        None
    ).await?;

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "ip": 1, "timestamp": 1 })
            .build(),
        None
    ).await?;
    
    Ok(())
}