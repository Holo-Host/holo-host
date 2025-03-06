use serde::{Deserialize, Serialize};
use super::shared::{
    meta::meta_validator,
    meta::Meta
};

pub const USER_COLLECTION_NAME: &str = "users";
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub oid: Option<bson::oid::ObjectId>,
    #[serde(rename = "_meta")]
    pub meta: Meta,

    // used for invalidating all refresh tokens for a user
    pub refresh_token_version: i32,

    // permissions for the user
    pub permissions: Vec<String>,

    // roles for the user
    pub roles: Vec<String>,
}

pub fn user_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["_meta", "refresh_token_version", "permissions", "roles"],
        "properties": {
            "_meta": meta_validator(),
            "refresh_token_version": {
                "bsonType": "int"
            },
            "permissions": {
                "bsonType": "array",
                "items": {
                    "bsonType": "string"
                }
            },
            "roles": {
                "bsonType": "array",
                "items": {
                    "bsonType": "string"
                }
            }
        }
    }
}