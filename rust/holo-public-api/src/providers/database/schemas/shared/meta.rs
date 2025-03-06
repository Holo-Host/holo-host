use bson::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Meta {
    pub is_deleted: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub deleted_at: Option<DateTime>,
}
pub fn meta_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["is_deleted", "created_at", "updated_at"],
        "properties": {
            "is_deleted": {
                "bsonType": "bool"
            },
            "created_at": {
                "bsonType": "date"
            },
            "updated_at": {
                "bsonType": "date"
            },
            "deleted_at": {
                "bsonType": ["date", "null"]
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MetaDto {
    pub is_deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

pub fn new_meta() -> Meta {
    Meta {
        is_deleted: false,
        created_at: DateTime::now(),
        updated_at: DateTime::now(),
        deleted_at: None,
    }
}

pub fn meta_to_dto(meta: Meta) -> Result<MetaDto, anyhow::Error> {
    Ok(MetaDto {
        is_deleted: meta.is_deleted,
        created_at: meta.created_at.try_to_rfc3339_string()?,
        updated_at: meta.updated_at.try_to_rfc3339_string()?,
        deleted_at: meta.deleted_at.map(|dt| match dt.try_to_rfc3339_string() {
            Ok(dt) => Some(dt),
            Err(_) => None,
        }).flatten(),
    })
}

pub fn meta_from_dto(meta_dto: MetaDto) -> Result<Meta, anyhow::Error> {
    let created_at = DateTime::parse_rfc3339_str(&meta_dto.created_at)?;
    let updated_at = DateTime::parse_rfc3339_str(&meta_dto.updated_at)?;
    let deleted_at = meta_dto.deleted_at.map(|dt| match DateTime::parse_rfc3339_str(&dt) {
        Ok(dt) => Some(dt),
        Err(_) => None,
    }).flatten();

    Ok(Meta {
        is_deleted: meta_dto.is_deleted,
        created_at,
        updated_at,
        deleted_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_to_dto() {
        let meta = Meta {
            is_deleted: false,
            created_at: DateTime::now(),
            updated_at: DateTime::now(),
            deleted_at: None,
        };

        let meta_dto = meta_to_dto(meta.clone()).unwrap();
        assert_eq!(meta_dto.is_deleted, meta.is_deleted);
        assert_eq!(meta_dto.created_at, meta.created_at.try_to_rfc3339_string().unwrap());
        assert_eq!(meta_dto.updated_at, meta.updated_at.try_to_rfc3339_string().unwrap());
        assert_eq!(meta_dto.deleted_at, None);
    }

    #[test]
    fn test_meta_from_dto() {
        let meta_dto = MetaDto {
            is_deleted: false,
            created_at: DateTime::now().try_to_rfc3339_string().unwrap(),
            updated_at: DateTime::now().try_to_rfc3339_string().unwrap(),
            deleted_at: None,
        };

        let meta = meta_from_dto(meta_dto.clone()).unwrap();
        assert_eq!(meta.is_deleted, meta_dto.is_deleted);
        assert_eq!(meta.created_at, DateTime::parse_rfc3339_str(&meta_dto.created_at).unwrap());
        assert_eq!(meta.updated_at, DateTime::parse_rfc3339_str(&meta_dto.updated_at).unwrap());
        assert_eq!(meta.deleted_at, None);
    }
}