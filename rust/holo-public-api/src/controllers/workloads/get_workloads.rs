use bson::doc;
use mongodb::Database;
use utoipa::OpenApi;

use crate::providers::{
    self, database::schemas,
    error_response::ErrorResponse,
    pagination::{Pagination, PaginationQuery}, permissions::get_claims_from_req
};

use actix_web::{
    get, web, HttpRequest, HttpResponse, Responder
};

#[derive(OpenApi)]
#[openapi(
    paths(get_workloads),
    components(schemas(Pagination<schemas::workload::WorkloadDto>))
)]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/protected/v1/workloads",
    summary = "Get all workloads",
    description = "Get all workloads accessible to the user",
    tag = "Workloads",
    security(
        ("Bearer" = [])
    ),
    params(
        PaginationQuery
    ),
    responses(
        (status = 200, body = Pagination<schemas::workload::WorkloadDto>)
    )
)]
#[get("/v1/workloads")]
pub async fn get_workloads(
    req: HttpRequest,
    query: web::Query<providers::pagination::PaginationQuery>,
    db: web::Data<Database>
) -> impl Responder {
    let page = query.page;
    let limit = query.limit;

    // validate page and limit
    if page < 1 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Page must be greater than 0".to_string()
        });
    }
    if limit < 0 || limit > 100 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Limit must be between 0 and 100".to_string()
        });
    }

    let claims = match get_claims_from_req(req) {
        Some(claims) => claims,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Unauthorized".to_string()
            });
        }
    };
    // verify user has permission to read workloads
    let permission = providers::permissions::verify_user_has_permission(
        claims.clone(),
        vec![ // required permissions
            providers::permissions::WORKLOADS_READ_ALL.to_string(),
            providers::permissions::WORKLOADS_READ.to_string(),
        ]
    );
    if permission.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "You do not have permission to access this resource".to_string()
        });
    }

    // only fetch workloads owned by user if they don't have all workloads read permission
    let mut filter: bson::Document = doc!{
        "_meta.is_deleted": false
    };
    let permission = permission.unwrap();
    if permission == providers::permissions::WORKLOADS_READ.to_string() {
        // fetch workloads owned by user
        filter = doc!{
            "_meta.is_deleted": false,
            "owner_user_id": bson::oid::ObjectId::parse_str(&claims.sub).unwrap()
        };
    }
    
    // fetch workloads
    let workloads_cursor = match db.collection::<schemas::workload::Workload>(
        schemas::workload::WORKLOAD_COLLECTION_NAME
    ).aggregate(
            vec![
                doc!{
                    "$match": filter.clone()
                },
                doc!{
                    "$skip": (page - 1) * limit
                },
                doc!{
                    "$limit": limit
                }
            ],
            None
        ).await {
        Ok(cursor) => cursor,
        Err(e) => {
            tracing::error!("Error getting workloads: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: format!("Error getting workloads")
            });
        }
    };
    // convert cursor to vec
    let workloads = match providers::database::cursor_to_vec::<schemas::workload::Workload>
        (workloads_cursor).await {
        Ok(workloads) => workloads,
        Err(e) => {
            tracing::error!("Error getting workloads: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: format!("Error getting workloads")
            });
        }
    };

    // count total workloads
    let total = match db.collection::<schemas::workload::Workload>(
        schemas::workload::WORKLOAD_COLLECTION_NAME
    )
        .count_documents(filter, None).await {
        Ok(total) => total,
        Err(e) => {
            tracing::error!("Error getting total workloads: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: format!("Error getting total workloads")
            });
        }
    };

    // convert workloads to dtos
    let items = match workloads.into_iter()
        .map(schemas::workload::workload_to_dto)
        .collect::<Result<Vec<_>, _>>() {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("Error converting workloads to dtos: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: format!("Error converting workloads to dtos")
            });
        }
    };

    let response: Pagination<schemas::workload::WorkloadDto> = Pagination {
        items,
        total: total as i32,
        page,
        limit,
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}