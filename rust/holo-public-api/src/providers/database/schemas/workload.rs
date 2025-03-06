use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::shared::{
    meta::{
        meta_from_dto,
        meta_to_dto,
        meta_validator,
        Meta,
        MetaDto
    },
    system_specs::{
        system_spec_from_dto,
        system_spec_to_dto,
        system_spec_validator,
        SystemSpec,
        SystemSpecDto
    }
};
pub fn workload_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["_meta", "owner_user_id", "version", "nix_pkg", "min_hosts", "system_specs"],
        "properties": {
            "_meta": meta_validator(),
            "owner_user_id": {
                "bsonType": "objectId"
            },
            "version": {
                "bsonType": "string"
            },
            "nix_pkg": {
                "bsonType": "string"
            },
            "min_hosts": {
                "bsonType": "int"
            },
            "system_specs": system_spec_validator()
        }
    }
}

pub const WORKLOAD_COLLECTION_NAME: &str = "workloads";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workload {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub oid: Option<bson::oid::ObjectId>,
    #[serde(rename = "_meta")]
    pub meta: Meta,
    pub owner_user_id: bson::oid::ObjectId,
    pub version: String,
    pub nix_pkg: String,
    pub min_hosts: i32,
    pub system_specs: SystemSpec,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct WorkloadDto {
    pub id: Option<String>,
    pub meta: MetaDto,
    pub owner_user_id: String,
    pub version: String,
    pub nix_pkg: String,
    pub min_hosts: i32,
    pub system_specs: SystemSpecDto,
}

pub async fn setup_workload_indexes(database: &mongodb::Database) -> Result<(), anyhow::Error> {
    // setup indexes
    let collection = database.collection::<Workload>(WORKLOAD_COLLECTION_NAME);
    collection.create_index(
        mongodb::IndexModel::builder().keys(
            bson::doc!{ "owner_user_id": 1 }
        ).build(),
        None
    ).await?;
    
    Ok(())
}

pub fn workload_to_dto(workload: Workload) -> Result<WorkloadDto, anyhow::Error> {
    Ok(WorkloadDto {
        id: workload.oid.map(|id| id.to_hex()),
        meta: meta_to_dto(workload.meta)?,
        owner_user_id: workload.owner_user_id.to_hex(),
        version: workload.version,
        nix_pkg: workload.nix_pkg,
        min_hosts: workload.min_hosts,
        system_specs: system_spec_to_dto(workload.system_specs)?,
    })
}

pub fn workload_from_dto(workload_dto: WorkloadDto) -> Result<Workload, anyhow::Error> {
    let id: Option<bson::oid::ObjectId> = match workload_dto.id {
        Some(id) => match bson::oid::ObjectId::parse_str(&id) {
            Ok(oid) => Some(oid),
            Err(e) => {
                return Err(anyhow::anyhow!("Invalid workload id: {}", e));
            }
        },
        None => None,
    };
    let meta = meta_from_dto(workload_dto.meta)?;
    let owner_user_id = bson::oid::ObjectId::parse_str(&workload_dto.owner_user_id)?;
    let version = workload_dto.version;
    let nix_pkg = workload_dto.nix_pkg;
    let min_hosts = workload_dto.min_hosts;
    let system_specs = system_spec_from_dto(workload_dto.system_specs);

    Ok(Workload {
        oid: id,
        meta,
        owner_user_id: owner_user_id,
        version: version,
        nix_pkg: nix_pkg,
        min_hosts: min_hosts,
        system_specs: system_specs,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workload_to_dto() {
        let workload = Workload {
            oid: Some(bson::oid::ObjectId::new()),
            meta: Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            },
            owner_user_id: bson::oid::ObjectId::new(),
            version: "1.0.0".to_string(),
            nix_pkg: "nix_pkg".to_string(),
            min_hosts: 1,
            system_specs: SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1,
            },
        };

        let workload_dto = workload_to_dto(workload.clone()).unwrap();
        assert_eq!(workload_dto.id, workload.oid.map(|id| id.to_hex()));
        assert_eq!(workload_dto.owner_user_id, workload.owner_user_id.to_hex());
        assert_eq!(workload_dto.version, workload.version);
        assert_eq!(workload_dto.nix_pkg, workload.nix_pkg);
        assert_eq!(workload_dto.min_hosts, workload.min_hosts);
    }

    #[test]
    fn test_workload_from_dto() {
        let workload_dto = WorkloadDto {
            id: Some(bson::oid::ObjectId::new().to_hex()),
            meta: meta_to_dto(Meta {
                is_deleted: false,
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
            }).unwrap(),
            owner_user_id: bson::oid::ObjectId::new().to_hex(),
            version: "1.0.0".to_string(),
            nix_pkg: "nix_pkg".to_string(),
            min_hosts: 1,
            system_specs: system_spec_to_dto(SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1,
            }).unwrap(),
        };

        let workload = workload_from_dto(workload_dto.clone()).unwrap();
        assert_eq!(workload.oid.map(|id| id.to_hex()), workload_dto.id);
        assert_eq!(workload.owner_user_id.to_hex(), workload_dto.owner_user_id);
        assert_eq!(workload.version, workload_dto.version);
        assert_eq!(workload.nix_pkg, workload_dto.nix_pkg);
        assert_eq!(workload.min_hosts, workload_dto.min_hosts);
    }
}
