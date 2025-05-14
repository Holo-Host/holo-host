use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user_permissions::PermissionAction,
};
use utoipa::OpenApi;

use crate::{
    controllers::apikey::apikey_dto::{map_api_key_to_dto, ApiKeyDto},
    providers::{
        self,
        error_response::ErrorResponse,
        jwt::AccessTokenClaims,
        pagination::{PaginationRequest, PaginationResponse},
    },
};

#[derive(OpenApi)]
#[openapi(paths(get_multiple_apikey))]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/protected/v1/apikeys",
    tag = "Apikey",
    summary = "Retrieve multiple API keys",
    description = "Requires 'api_key.Read' permission",
    security(
        ("Bearer" = [])
    ),
    params(
        ("page" = i32, Query, description = "The page number to return"),
        ("limit" = i32, Query, description = "The number of items to return per page")
    ),
    responses(
        (status = 200, body = PaginationResponse<ApiKeyDto>)
    )
)]
#[get("/v1/apikeys")]
pub async fn get_multiple_apikey(
    req: HttpRequest,
    query: web::Query<PaginationRequest>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    // get pagination parameters
    let PaginationRequest { page, limit } = query.into_inner();
    if page < 1 || limit < 1 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Page and limit must be greater than 0".to_string(),
        });
    }
    if limit > 100 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Limit must be less than or equal to 100".to_string(),
        });
    }

    // get all owners that the user has access to
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();
    let owners = providers::auth::get_all_accessible_owners_from_permissions(
        claims.permissions,
        API_KEY_COLLECTION_NAME.to_string(),
        PermissionAction::Read,
        claims.sub,
    );

    // get a page if items accessible by the user from the database
    let api_keys = match providers::crud::get_many::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        Some(bson::doc! {
            "owner": {
                "$in": owners.clone()
            }
        }),
        None,
        Some(limit),
        Some((page - 1) * limit),
    )
    .await
    {
        Ok(api_keys) => api_keys,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    // get the total amount of api keys accessible by the user
    let total_count = match providers::crud::count::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        Some(bson::doc! {
            "owner": {
                "$in": owners.clone()
            }
        }),
    )
    .await
    {
        Ok(count) => count,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    // map api keys to DTOs and return Pagination Response
    let api_keys_mapped = api_keys.into_iter().map(map_api_key_to_dto).collect();
    let result: PaginationResponse<ApiKeyDto> = PaginationResponse {
        total: total_count,
        page,
        limit,
        items: api_keys_mapped,
    };
    HttpResponse::Ok().json(result)
}
