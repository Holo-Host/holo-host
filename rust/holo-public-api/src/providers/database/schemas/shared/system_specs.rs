use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct SystemSpec {
    pub memory: i32,
    pub disk: i32,
    pub cores: i32,
}

pub fn system_spec_validator() -> bson::Document {
    bson::doc!{
        "bsonType": "object",
        "required": ["memory", "disk", "cores"],
        "properties": {
            "memory": {
                "bsonType": "int"
            },
            "disk": {
                "bsonType": "int"
            },
            "cores": {
                "bsonType": "int"
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
pub struct SystemSpecDto {
    pub memory: i32,
    pub disk: i32,
    pub cores: i32,
}

pub fn system_spec_to_dto(system_spec: SystemSpec) -> Result<SystemSpecDto, anyhow::Error> {
    Ok(SystemSpecDto {
        memory: system_spec.memory,
        disk: system_spec.disk,
        cores: system_spec.cores,
    })
}

pub fn system_spec_from_dto(system_spec_dto: SystemSpecDto) -> SystemSpec {
    SystemSpec {
        memory: system_spec_dto.memory,
        disk: system_spec_dto.disk,
        cores: system_spec_dto.cores,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_spec_to_dto() {
        let system_spec = SystemSpec {
            memory: 1024,
            disk: 1024,
            cores: 1,
        };

        let system_spec_dto = system_spec_to_dto(system_spec.clone()).unwrap();
        assert_eq!(system_spec_dto.memory, system_spec.memory);
        assert_eq!(system_spec_dto.disk, system_spec.disk);
        assert_eq!(system_spec_dto.cores, system_spec.cores);
    }

    #[test]
    fn test_system_spec_from_dto() {
        let system_spec_dto = SystemSpecDto {
            memory: 1024,
            disk: 1024,
            cores: 1,
        };

        let system_spec = system_spec_from_dto(system_spec_dto.clone());
        assert_eq!(system_spec.memory, system_spec_dto.memory);
        assert_eq!(system_spec.disk, system_spec_dto.disk);
        assert_eq!(system_spec.cores, system_spec_dto.cores);
    }
}
