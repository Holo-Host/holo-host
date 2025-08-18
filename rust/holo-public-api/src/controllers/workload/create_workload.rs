use crate::{controllers::workload::workload_dto, providers};
use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas;
use utoipa::OpenApi;

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
    request_body = workload_dto::CreateWorkloadDto,
    responses(
        (status = 200, body = workload_dto::WorkloadDto)
    )
)]
#[post("/v1/workload")]
pub async fn create_workload(
    req: HttpRequest,
    payload: web::Json<workload_dto::WorkloadDto>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let payload = payload.into_inner();
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
    let user_id = match ObjectId::parse_str(claims.sub.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
                message: "Permission denied".to_string(),
            });
        }
    };

    // Verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Create,
            owner: claims.sub,
        }],
    ) {
        return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::create(
        db.as_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        schemas::workload::Workload {
            _id: ObjectId::new(),
            metadata: schemas::metadata::Metadata::default(),
            owner: user_id,
            bootstrap_server_url: payload.bootstrap_server_url.clone(),
            signal_server_url: payload.signal_server_url.clone(),
            http_gw_enable: payload.http_gw_enable,
            http_gw_allowed_fns: payload.http_gw_allowed_fns.clone(),
            memproof: payload.memproof.clone(),
            network_seed: payload.network_seed.clone(),
            execution_policy: workload_dto::execution_policy_from_dto(
                payload.execution_policy.clone(),
            ),
        },
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

    HttpResponse::Ok().json(workload_dto::WorkloadDto {
        id: result.to_hex(),
        bootstrap_server_url: payload.bootstrap_server_url,
        signal_server_url: payload.signal_server_url,
        http_gw_enable: payload.http_gw_enable,
        http_gw_allowed_fns: payload.http_gw_allowed_fns,
        memproof: payload.memproof,
        network_seed: payload.network_seed,
        execution_policy: payload.execution_policy,
    })
}
