use serde::{Deserialize, Serialize};
use bson::oid::ObjectId;
use utoipa::ToSchema;
use crate::providers::database::schemas::shared::meta::meta_validator;

use super::shared::meta::{meta_from_dto, meta_to_dto, Meta, MetaDto};

pub const USER_INFO_COLLECTION_NAME: &str = "user_infos";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub _meta: Meta,
    
    pub user_id: ObjectId,
    pub given_name: String,
    pub family_name: String,
    pub phone_number: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct UserInfoDto {
    pub id: Option<String>,
    pub meta: MetaDto,
    pub user_id: String,
    pub given_name: String,
    pub family_name: String,
    pub phone_number: String,
    pub email: String,
}

pub fn user_info_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["_meta", "user_id", "given_name", "family_name", "phone_number", "email"],
        "properties": {
            "_meta": meta_validator(),
            "user_id": {
                "bsonType": "objectId"
            },
            "given_name": {
                "bsonType": "string"
            },
            "family_name": {
                "bsonType": "string"
            },
            "phone_number": {
                "bsonType": "string"
            },
            "email": {
                "bsonType": "string"
            }
        }
    }
}

pub fn user_info_to_dto(user_info: UserInfo) -> Result<UserInfoDto, anyhow::Error> {
    Ok(UserInfoDto {
        id: user_info._id.map(|id| id.to_hex()),
        meta: meta_to_dto(user_info._meta)?,
        user_id: user_info.user_id.to_hex(),
        given_name: user_info.given_name,
        family_name: user_info.family_name,
        phone_number: user_info.phone_number,
        email: user_info.email,
    })
}

pub fn user_info_from_dto(user_info_dto: UserInfoDto) -> Result<UserInfo, anyhow::Error> {
    let id = match user_info_dto.id.map(|id| bson::oid::ObjectId::parse_str(id)) {
        Some(Ok(id)) => Some(id),
        Some(Err(e)) => return Err(anyhow::anyhow!(e)),
        None => None,
    };
    let meta = meta_from_dto(user_info_dto.meta)?;
    let user_id = bson::oid::ObjectId::parse_str(&user_info_dto.user_id)?;
    let given_name = user_info_dto.given_name;
    let family_name = user_info_dto.family_name;
    let phone_number = user_info_dto.phone_number;
    let email = user_info_dto.email;

    Ok(UserInfo {
        _id: id,
        _meta: meta,
        user_id: user_id,
        given_name: given_name,
        family_name: family_name,
        phone_number: phone_number,
        email: email,
    })
}

pub async fn setup_user_info_indexes(database: &mongodb::Database) -> Result<(), anyhow::Error> {
    let collection = database.collection::<UserInfo>(USER_INFO_COLLECTION_NAME);

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "user_id": 1 })
            .build(),
        None
    ).await?;

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "email": 1 })
            .build(),
        None
    ).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_info_to_dto() {
        let user_info = UserInfo {
            _id: Some(ObjectId::new()),
            _meta: Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            },
            user_id: ObjectId::new(),
            given_name: "John".to_string(),
            family_name: "Doe".to_string(),
            phone_number: "+1234567890".to_string(),
            email: "john.doe@example.com".to_string(),
        };

        let user_info_dto = user_info_to_dto(user_info.clone()).unwrap();
        assert_eq!(user_info_dto.id, user_info._id.map(|id| id.to_hex()));
        assert_eq!(user_info_dto.user_id, user_info.user_id.to_hex());
        assert_eq!(user_info_dto.given_name, user_info.given_name);
        assert_eq!(user_info_dto.family_name, user_info.family_name);
        assert_eq!(user_info_dto.phone_number, user_info.phone_number);
        assert_eq!(user_info_dto.email, user_info.email);
    }

    #[test]
    fn test_user_info_from_dto() {
        let user_info_dto = UserInfoDto {
            id: Some(ObjectId::new().to_hex()),
            meta: meta_to_dto(Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            }).unwrap(),
            user_id: ObjectId::new().to_hex(),
            given_name: "John".to_string(),
            family_name: "Doe".to_string(),
            phone_number: "+1234567890".to_string(),
            email: "john.doe@example.com".to_string(),
        };

        let user_info = user_info_from_dto(user_info_dto.clone()).unwrap();
        assert_eq!(user_info._id.map(|id| id.to_hex()), user_info_dto.id);
        assert_eq!(user_info.user_id.to_hex(), user_info_dto.user_id);
        assert_eq!(user_info.given_name, user_info_dto.given_name);
        assert_eq!(user_info.family_name, user_info_dto.family_name);
        assert_eq!(user_info.phone_number, user_info_dto.phone_number);
        assert_eq!(user_info.email, user_info_dto.email);
    }
}