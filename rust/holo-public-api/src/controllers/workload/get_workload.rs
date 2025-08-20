use crate::{controllers::workload::workload_dto, providers};
use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::{self, workload::Context};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(get_workload))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/workload",
    tag = "Workload",
    summary = "Create manifiest",
    description = "Requires 'workload.Read' permission",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, body = workload_dto::WorkloadDto)
    )
)]
#[get("/v1/workload/{id}")]
pub async fn get_workload(
    req: HttpRequest,
    id: web::Path<String>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let claims = req
        .extensions()
        .get::<providers::jwt::AccessTokenClaims>()
        .cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(providers::error_response::ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    let owner = match providers::crud::get_owner::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to get workload".to_string(),
                },
            );
        }
    };
    if owner.is_none() {
        return HttpResponse::NotFound().json(providers::error_response::ErrorResponse {
            message: "no workload found with the given id".to_string(),
        });
    }
    let owner = owner.unwrap();

    // Verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Read,
            owner,
        }],
    ) {
        return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::get::<schemas::workload::Workload>(
        db.as_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to create workload".to_string(),
                },
            );
        }
    };
    if result.is_none() {
        return HttpResponse::NotFound().json(providers::error_response::ErrorResponse {
            message: "no workload found with the given id".to_string(),
        });
    }
    let result = result.unwrap();

    HttpResponse::Ok().json(workload_dto::WorkloadDto {
        id: result._id.to_hex(),
        manifest_id: result.manifest_id.to_hex(),
        execution_policy: workload_dto::execution_policy_to_dto(result.execution_policy),
        http_gw_enable: result.context.http_gw_enable,
        http_gw_allowed_fns: result.context.http_gw_allowed_fns,
        network_seed: result.context.network_seed,
        // bootstrap_server_url: result.bootstrap_server_url,
        // signal_server_url: result.signal_server_url,
        // memproof: result.memproof,
    })
}
