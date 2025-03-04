use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use mongodb::Database;
use utoipa::OpenApi;

use crate::providers::{
    database::schemas::workload::{
        self,
        Workload,
        WorkloadDto
    },
    error_response::ErrorResponse,
    permissions::{
        get_claims_from_req, verify_user_has_permission, WORKLOADS_READ, WORKLOADS_READ_ALL
    }
};

#[derive(OpenApi)]
#[openapi(
    paths(get_single_workload),
    components(schemas(WorkloadDto))
)]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/protected/v1/workload/{id}",
    tag = "Workloads",
    summary = "Get a single workload",
    description = "Get a single workload by id",
    security(
        ("Bearer" = [])
    ),
    params(
        ("id" = String, Path, description = "The id of the workload to get")
    ),
    responses(
        (status = 200, body = WorkloadDto)
    )
)]
#[get("/v1/workload/{id}")]
pub async fn get_single_workload(
    req: HttpRequest,
    db: web::Data<Database>,
    path: web::Path<String>,
) -> impl Responder {
    let claims = match get_claims_from_req(req) {
        Some(claims) => claims,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Unauthorized".to_string()
            });
        }
    };
    let permission =verify_user_has_permission(
        claims.clone(),
        vec![
            WORKLOADS_READ_ALL.to_string(),
            WORKLOADS_READ.to_string(),
        ]
    );
    if permission.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "You do not have permission to access this resource".to_string(),
        });
    }
    let workload_id = path.into_inner();
    let workload_id_oid = match bson::oid::ObjectId::parse_str(&workload_id) {
        Ok(oid) => oid,
        Err(e) => {
            tracing::error!("Error parsing workload id: {}", e);
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "Invalid workload id".to_string(),
            });
        }
    };
    let filter: bson::Document;
    if permission.unwrap() == WORKLOADS_READ {
        filter = bson::doc! {
            "_id": workload_id_oid,
            "owner_user_id": bson::oid::ObjectId::parse_str(&claims.sub).unwrap(),
            "_meta.is_deleted": false
        };
    } else {
        filter = bson::doc! {
            "_id": workload_id_oid,
            "_meta.is_deleted": false
        };
    }

    let workload = match db.collection::<Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).find_one(filter, None).await {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("Error getting workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error getting workload".to_string(),
            });
        }
    };

    if workload.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Workload not found".to_string(),
        });
    }

    let workload_dto = match workload::workload_to_dto(workload.unwrap()) {
        Ok(workload_dto) => workload_dto,
        Err(e) => {
            tracing::error!("Error converting workload to dto: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error converting workload to dto".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(workload_dto)
}