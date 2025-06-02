use crate::{
    controllers::workload_layout::workload_layout_dto::{
        from_workload_layout_dto, CreateWorkloadLayoutDto,
    },
    providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims},
};
use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(create_workload))]
pub struct OpenApiSpec;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, utoipa::ToSchema)]
pub struct CreateWorkloadLayoutResponse {
    pub id: String,
}

#[utoipa::path(
    post,
    path = "/protected/v1/workload-layout",
    tag = "Workload",
    summary = "Create workload layout",
    description = "Requires 'workload_layout.Create' permission",
    security(
        ("Bearer" = [])
    ),
    request_body = CreateWorkloadLayoutDto,
    responses(
        (status = 200, body = CreateWorkloadLayoutResponse)
    )
)]
#[post("/v1/workload-layout")]
pub async fn create_workload_layout(
    req: HttpRequest,
    payload: web::Json<CreateWorkloadLayoutDto>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::workload_layout::WORKLOAD_LAYOUT_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Read,
            owner: claims.sub.clone(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::create::<schemas::workload_layout::WorkloadLayout>(
        db.get_ref().clone(),
        schemas::workload_layout::WORKLOAD_LAYOUT_COLLECTION_NAME.to_string(),
        from_workload_layout_dto(),
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
