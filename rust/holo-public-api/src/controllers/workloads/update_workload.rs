use actix_web::{patch, web, HttpRequest, HttpResponse, Responder};
use mongodb::Database;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use crate::providers::{
    database::schemas::{user, workload},
    error_response::ErrorResponse,
    permissions::{
        get_claims_from_req, verify_user_has_permission, WORKLOADS_UPDATE, WORKLOADS_UPDATE_ALL
    }
};

#[derive(OpenApi)]
#[openapi(
    paths(update_workload),
    components(schemas(UpdateWorkloadRequest))
)]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateWorkloadRequestSystemSpecs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<i32>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateWorkloadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nix_pkg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_hosts: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_specs: Option<UpdateWorkloadRequestSystemSpecs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_user_id: Option<String>,
}

#[utoipa::path(
    patch,
    path = "/protected/v1/workload/{id}",
    summary = "Update an existing workload",
    description = "Update an existing workload",
    tag = "Workloads",
    security(
        ("Bearer" = [])
    ),
    request_body = UpdateWorkloadRequest,
    params(
        ("id" = String, Path, description = "The id of the workload to update")
    ),
    responses(
        (status = 200)
    )
)]
#[patch("/v1/workload/{id}")]
pub async fn update_workload(
    req: HttpRequest,
    db: web::Data<Database>,
    path: web::Path<String>,
    body: web::Json<UpdateWorkloadRequest>,
) -> impl Responder {
    let update_workload_request = body.into_inner();
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
            WORKLOADS_UPDATE_ALL.to_string(),
            WORKLOADS_UPDATE.to_string(),
        ]
    );
    if permission.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "You do not have permission to delete workloads".to_string(),
        });
    }

    // create filter
    let workload_id = match bson::oid::ObjectId::parse_str(path.into_inner()) {
        Ok(workload_id) => workload_id,
        Err(e) => {
            tracing::error!("Error parsing workload id: {}", e);
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "Invalid workload id".to_string(),
            });
        }
    };
    let filter: bson::Document;
    if permission.unwrap() == WORKLOADS_UPDATE_ALL.to_string() {
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

    // fetch workload to be updated
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

    // if owner_user_id is provided, verify new user exists
    if let Some(owner_user_id) = update_workload_request.owner_user_id.clone() {
        let owner_user_id = match bson::oid::ObjectId::parse_str(&owner_user_id) {
            Ok(owner_user_id) => owner_user_id,
            Err(e) => {
                tracing::error!("Error parsing owner user id: {}", e);
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "Invalid owner user id".to_string(),
                });
            }
        };
        let user = match db.collection::<user::User>(
            user::USER_COLLECTION_NAME
        ).find_one(bson::doc!{ "_id": owner_user_id }, None).await {
            Ok(user) => user,
            Err(e) => {
                tracing::error!("Error fetching user: {}", e);
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "Error fetching user".to_string(),
                });
            }
        };
        if user.is_none() {
            return HttpResponse::NotFound().json(ErrorResponse {
                message: "New owner user not found".to_string(),
            });
        }
    }

    let update_payload = match bson::to_bson(&update_workload_request) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::error!("Error converting update workload request to bson: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error converting update workload request to bson".to_string(),
            });
        }
    };

    // update workload
    let update = bson::doc!{
        "$set": update_payload,
    };
    let result = db.collection::<workload::Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).update_one(
        bson::doc!{ "_id": workload.oid },
        update,
        None
    ).await;

    if result.is_err() {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Error deleting workload".to_string(),
        });
    }

    let updated_workload = match db.collection::<workload::Workload>(
        workload::WORKLOAD_COLLECTION_NAME
    ).find_one(bson::doc!{ "_id": workload.oid }, None).await {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("Error fetching workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error fetching workload".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(updated_workload)
}