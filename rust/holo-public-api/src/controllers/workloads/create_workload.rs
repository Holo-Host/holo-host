use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{
    database::schemas::{
        shared::{
            meta::Meta,
            system_specs::{self, SystemSpecDto}
        },
        workload
    },
    error_response::ErrorResponse,
    permissions::{
        get_claims_from_req, verify_user_has_permission, WORKLOADS_CREATE
    }
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadRequest {
    pub version: String,
    pub nix_pkg: String,
    pub min_hosts: i32,
    pub system_specs: SystemSpecDto,
}

#[derive(OpenApi)]
#[openapi(
    paths(create_workload),
    components(schemas(CreateWorkloadRequest))
)]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/workload",
    summary = "Create a new workload",
    description = "Create a new workload owned by the logged in user",
    tag = "Workloads",
    security(
        ("Bearer" = [])
    ),
    request_body = CreateWorkloadRequest,
    responses(
        (status = 200, body = workload::WorkloadDto)
    )
)]
#[post("/v1/workload")]
pub async fn create_workload(
    req: HttpRequest,
    db: web::Data<Database>,
    workload: web::Json<CreateWorkloadRequest>,
) -> impl Responder {
    let claims = match get_claims_from_req(req) {
        Some(claims) => claims,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Unauthorized".to_string()
            });
        }
    };

    let permission = verify_user_has_permission(
        claims.clone(),
        vec![
            WORKLOADS_CREATE.to_string()
        ]
    );
    if permission.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "You are not allowed to create workloads".to_string(),
        });
    }

    let workload_request = workload.into_inner();
    let mut workload = workload::Workload {
        oid: None,
        meta: Meta {
            created_at: bson::DateTime::now(),
            updated_at: bson::DateTime::now(),
            deleted_at: None,
            is_deleted: false,
        },
        owner_user_id: bson::oid::ObjectId::parse_str(&claims.sub).unwrap(),
        version: workload_request.version,
        nix_pkg: workload_request.nix_pkg,
        min_hosts: workload_request.min_hosts,
        system_specs: system_specs::system_spec_from_dto(workload_request.system_specs),
    };

    let result = match db.collection::<workload::Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).insert_one(workload.clone(), None).await {
        Ok(result) => {
            workload.oid = Some(result.inserted_id.as_object_id().unwrap());
            workload
        },
        Err(e) => {
            tracing::error!("Error creating workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error creating workload".to_string(),
            })
        }
    };

    let workload_dto = match workload::workload_to_dto(result) {
        Ok(workload_dto) => workload_dto,
        Err(e) => {
            tracing::error!("Error converting workload to dto: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error converting workload to dto".to_string(),
            })
        }
    };

    HttpResponse::Created().json(workload_dto)
}