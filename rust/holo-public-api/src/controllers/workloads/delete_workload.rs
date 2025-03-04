use actix_web::{delete, web, HttpRequest, HttpResponse, Responder};
use mongodb::Database;
use utoipa::OpenApi;
use crate::providers::{
    database::schemas::workload, error_response::ErrorResponse, permissions::{
        get_claims_from_req, verify_user_has_permission, WORKLOADS_DELETE_ALL, WORKLOADS_DELETE
    }
};

#[derive(OpenApi)]
#[openapi(
    paths(delete_workload)
)]
pub struct OpenApiSpec;

#[utoipa::path(
    delete,
    path = "/protected/v1/workload/{id}",
    summary = "Delete a workload",
    description = "Delete a workload",
    tag = "Workloads",
    security(
        ("Bearer" = [])
    ),
    params(
        ("id" = String, Path, description = "The id of the workload to delete")
    ),
    responses(
        (status = 200)
    )
)]
#[delete("/v1/workload/{id}")]
pub async fn delete_workload(
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
    // verify user has permission to delete workloads
    let permission = verify_user_has_permission(
        claims.clone(),
        vec![
            WORKLOADS_DELETE_ALL.to_string(),
            WORKLOADS_DELETE.to_string(),
        ]
    );
    if permission.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "You do not have permission to delete workloads".to_string(),
        });
    }
    let workload_id = match bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Error parsing workload id: {}", e);
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "Invalid workload id".to_string(),
            });
        }
    };
    let filter: bson::Document;
    if permission.unwrap() == WORKLOADS_DELETE_ALL.to_string() {
        filter = bson::doc!{
            "_id": workload_id,
            "_meta.is_deleted": false
        };
    } else {
        filter = bson::doc!{
            "_id": workload_id,
            "owner_user_id": bson::oid::ObjectId::parse_str(&claims.sub).unwrap(),
            "_meta.is_deleted": false
        };
    }

    // fetch workload to be deleted
    let workload = match db.collection::<workload::Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).find_one(filter.clone(), None).await {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("Error fetching workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error fetching workload".to_string(),
            });
        }
    };
    if workload.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Workload not found".to_string(),
        });
    }
    let workload = workload.unwrap();

    // update workload to be deleted
    let update = bson::doc!{
        "$set": {
            "_meta.is_deleted": true,
            "_meta.deleted_at": bson::DateTime::now()
        }
    };
    let result = db.collection::<workload::Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).update_one(
        bson::doc!{ "_id": workload._id },
        update,
        None
    ).await;

    if result.is_err() {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Error deleting workload".to_string(),
        });
    }

    HttpResponse::Ok().finish()
}