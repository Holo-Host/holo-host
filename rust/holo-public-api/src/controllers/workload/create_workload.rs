use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas::{
    user_permissions::{PermissionAction, UserPermission},
    workload::{Workload, WORKLOAD_COLLECTION_NAME},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadRequest {
    /// the owner of this object, if not set then the logged in user is the owner
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,

    /// minimum number of hosts required for the workload
    #[schema(example = 1)]
    min_hosts: i32,

    /// minimum disk space required for the workload
    #[schema(example = 1)]
    min_disk_space: i32,

    /// minimum network speed required for the workload
    #[schema(example = 1)]
    min_cpu_cores: i32,

    /// average uptime required for the node
    #[schema(example = 0.8)]
    avg_uptime_required: f32,

    /// average network speed required for the node
    #[schema(example = 1)]
    avg_network_speed_required: i32,

    /// workload version (defined by the user)
    #[schema(example = "0.0.0")]
    version: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadResponse {
    pub id: String,
}

#[derive(OpenApi)]
#[openapi(paths(create_workload))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/workload",
    tag = "Workload",
    summary = "Create workload",
    description = "Requires 'workload.Create' permission",
    security(
        ("Bearer" = [])
    ),
    request_body = CreateWorkloadRequest,
    responses(
        (status = 200, body = CreateWorkloadResponse)
    )
)]
#[post("/v1/workload")]
pub async fn create_workload(
    req: HttpRequest,
    payload: web::Json<CreateWorkloadRequest>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    let owner = match payload.owner.clone() {
        Some(owner) => owner,
        None => claims.sub.clone(),
    };

    let owner_oid = match ObjectId::parse_str(owner.clone()) {
        Ok(oid) => oid,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "invalid owner id".to_string(),
            });
        }
    };

    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![UserPermission {
            resource: WORKLOAD_COLLECTION_NAME.to_string(),
            action: PermissionAction::Create,
            owner,
            all_owners: false,
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::create::<Workload>(
        db.get_ref().clone(),
        WORKLOAD_COLLECTION_NAME.to_string(),
        Workload {
            _id: None,
            owner: owner_oid,
            metadata: db_utils::schemas::metadata::Metadata::default(),
            assigned_hosts: vec![],
            manifest: db_utils::schemas::workload::WorkloadManifest::None,
            min_hosts: payload.min_hosts,
            status: db_utils::schemas::workload::WorkloadStatus {
                id: None,
                desired: db_utils::schemas::workload::WorkloadState::Reported,
                actual: db_utils::schemas::workload::WorkloadState::Reported,
                payload: db_utils::schemas::workload::WorkloadStatePayload::None,
            },
            system_specs: db_utils::schemas::workload::SystemSpecs {
                capacity: db_utils::schemas::workload::Capacity {
                    drive: payload.min_disk_space as i64,
                    cores: payload.min_cpu_cores as i64,
                },
                avg_network_speed: payload.avg_network_speed_required as i64,
                avg_uptime: payload.avg_uptime_required as f64,
            },
            version: payload.version.clone(),
        },
    )
    .await
    {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(CreateWorkloadResponse {
        id: result.to_hex(),
    })
}
