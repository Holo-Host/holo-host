use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::providers::database::schemas::shared::{meta::meta_validator, system_specs::system_spec_validator};

use super::shared::{
    meta::{
        meta_from_dto,
        meta_to_dto,
        Meta,
        MetaDto
    },
    system_specs::{
        system_spec_from_dto,
        system_spec_to_dto,
        SystemSpec,
        SystemSpecDto
    }
};

pub const HOST_COLLECTION_NAME: &str = "hosts";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Host {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<bson::oid::ObjectId>,
    pub _meta: Meta,
    
    pub owner_user_id: bson::oid::ObjectId,
    pub assigned_workloads: Vec<bson::oid::ObjectId>,
    pub system_specs: SystemSpec,
    pub ip_address: String,
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct HostDto {
    pub id: Option<String>,
    pub meta: MetaDto,
    pub owner_user_id: String,
    pub assigned_workloads: Vec<String>,
    pub system_specs: SystemSpecDto,
    pub ip_address: String,
    pub device_id: String,
}

pub fn host_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["_meta", "owner_user_id", "assigned_workloads", "system_specs", "ip_address", "device_id"],
        "properties": {
            "_meta": meta_validator(),
            "owner_user_id": {
                "bsonType": "objectId"
            },
            "assigned_workloads": {
                "bsonType": "array",
                "items": {
                    "bsonType": "objectId"
                }
            },
            "system_specs": system_spec_validator(),
            "ip_address": {
                "bsonType": "string"
            },
            "device_id": {
                "bsonType": "string"
            }
        }
    }
}


pub fn host_to_dto(host: Host) -> Result<HostDto, anyhow::Error> {
    Ok(HostDto {
        id: host._id.map(|id| id.to_hex()),
        meta: meta_to_dto(host._meta)?,
        owner_user_id: host.owner_user_id.to_hex(),
        assigned_workloads: host.assigned_workloads.iter().map(|id| id.to_hex()).collect(),
        system_specs: system_spec_to_dto(host.system_specs)?,
        ip_address: host.ip_address,
        device_id: host.device_id,
    })
}

pub fn host_from_dto(host_dto: HostDto) -> Result<Host, anyhow::Error> {
    let id = match host_dto.id.map(|id| bson::oid::ObjectId::parse_str(id)) {
        Some(Ok(id)) => Some(id),
        Some(Err(e)) => return Err(anyhow::anyhow!(e)),
        None => None,
    };
    let meta = meta_from_dto(host_dto.meta)?;
    let owner_user_id = bson::oid::ObjectId::parse_str(&host_dto.owner_user_id)?;
    let assigned_workloads = host_dto.assigned_workloads
        .iter()
        .map(|id| bson::oid::ObjectId::parse_str(id))
        .collect::<Result<Vec<bson::oid::ObjectId>, bson::oid::Error>>()?;
    let system_specs = system_spec_from_dto(host_dto.system_specs);
    let ip_address = host_dto.ip_address;
    let device_id = host_dto.device_id;

    Ok(Host {
        _id: id,
        _meta: meta,
        owner_user_id: owner_user_id,
        assigned_workloads: assigned_workloads,
        system_specs: system_specs,
        ip_address: ip_address,
        device_id: device_id,
    })
}

pub async fn setup_host_indexes(database: &mongodb::Database) -> Result<(), anyhow::Error> {
    let collection = database.collection::<Host>(HOST_COLLECTION_NAME);

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "owner_user_id": 1 })
            .build(),
        None
    ).await?;

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "ip_address": 1 })
            .build(),
        None
    ).await?;

    collection.create_index(
        mongodb::IndexModel::builder()
            .keys(bson::doc! { "device_id": 1 })
            .build(),
        None
    ).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_to_dto() {
        let host = Host {
            _id: Some(bson::oid::ObjectId::new()),
            _meta: Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            },
            owner_user_id: bson::oid::ObjectId::new(),
            assigned_workloads: vec![bson::oid::ObjectId::new()],
            system_specs: SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1,
            },
            ip_address: "192.168.1.1".to_string(),
            device_id: "device_id".to_string(),
        };

        let host_dto = host_to_dto(host.clone()).unwrap();
        assert_eq!(host_dto.id, host._id.map(|id| id.to_hex()));
        assert_eq!(host_dto.owner_user_id, host.owner_user_id.to_hex());
        assert_eq!(host_dto.assigned_workloads, vec![host.assigned_workloads[0].to_hex()]);
        assert_eq!(host_dto.ip_address, host.ip_address);
        assert_eq!(host_dto.device_id, host.device_id);
    }

    #[test]
    fn test_host_from_dto() {
        let host_dto = HostDto {
            id: Some(bson::oid::ObjectId::new().to_hex()),
            meta: meta_to_dto(Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            }).unwrap(),
            owner_user_id: bson::oid::ObjectId::new().to_hex(),
            assigned_workloads: vec![bson::oid::ObjectId::new().to_hex()],
            system_specs: system_spec_to_dto(SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1,
            }).unwrap(),
            ip_address: "192.168.1.1".to_string(),
            device_id: "device_id".to_string(),
        };

        let host = host_from_dto(host_dto.clone()).unwrap();
        assert_eq!(host._id.map(|id| id.to_hex()), host_dto.id);
        assert_eq!(host.owner_user_id, bson::oid::ObjectId::parse_str(&host_dto.owner_user_id).unwrap());
        assert_eq!(host.assigned_workloads, vec![bson::oid::ObjectId::parse_str(&host_dto.assigned_workloads[0]).unwrap()]);
        assert_eq!(host.ip_address, host_dto.ip_address);
        assert_eq!(host.device_id, host_dto.device_id);
    }
}
