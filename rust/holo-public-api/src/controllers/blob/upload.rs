use std::fs;
use std::io::Write;

use crate::providers::{
    auth::verify_all_permissions, config::AppConfig, error_response::ErrorResponse,
    jwt::AccessTokenClaims,
};
use actix_multipart::Multipart;
use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::user_permissions::{PermissionAction, UserPermission};
use futures_util::StreamExt;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(paths(upload_blob), components(schemas(UploadBlobResponse)))]
pub struct OpenApiSpec;

#[derive(Serialize, ToSchema)]
pub struct UploadBlobResponse {
    pub hash: String,
}

#[derive(Serialize, ToSchema)]
struct BlobUploadRequest {
    #[schema(value_type = String, format = "binary")]
    blob: Vec<u8>,
}

#[utoipa::path(
    post,
    path = "/protected/v1/blob/upload",
    tag = "Blob",
    summary = "Upload Blob",
    description = "Upload a blob",
    security(
        ("Bearer" = [])
    ),
    request_body(
        content = BlobUploadRequest,
        content_type = "multipart/form-data",
    ),
    responses(
        (status = 200, body = UploadBlobResponse)
    )
)]
#[post("/v1/blob/upload")]
pub async fn upload_blob(
    req: HttpRequest,
    mut payload: Multipart,
    config: web::Data<AppConfig>,
) -> impl Responder {
    // get user claims from the request
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();
    let owner = claims.sub.clone();

    // check if the user has permission to upload blobs
    if !verify_all_permissions(
        claims,
        vec![UserPermission {
            resource: "blob".to_string(),
            action: PermissionAction::Create,
            owner: owner.clone(),
            all_owners: false,
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "forbidden".to_string(),
        });
    }
    let temp_file_identifier = bson::uuid::Uuid::new().to_string();
    let mut hasher = blake3::Hasher::new();
    let mut blob = match fs::File::create(format!("/tmp/{}", temp_file_identifier)) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    // process the blob
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(e) => {
                tracing::error!("{:?}", e);
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "Invalid request".to_string(),
                });
            }
        };

        // Process each chunk in the field.
        while let Some(chunk) = field.next().await {
            let chunk = match chunk {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    return HttpResponse::BadRequest().json(ErrorResponse {
                        message: "Invalid request".to_string(),
                    });
                }
            };

            // Update hash and append to our accumulator.
            hasher.update(&chunk);
            match blob.write_all(&chunk) {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("{:?}", e);
                    return HttpResponse::InternalServerError().json(ErrorResponse {
                        message: "Internal server error".to_string(),
                    });
                }
            }
        }
    }

    // finalize the hash
    let hash_hex = hasher.finalize().to_hex().to_string();

    // create metadata body
    let metadata_body = bson::doc! {
        "createdAt": bson::DateTime::now().to_string(),
        "updatedAt": bson::DateTime::now().to_string(),
        "userId": owner.clone(),
        "hash": hash_hex.clone(),
    }
    .to_string()
    .into_bytes();

    // create directory for the blob and metadata
    let file_location = match config.blob_storage_location {
        Some(ref location) => location.clone(),
        None => ".".to_string(),
    };
    let exists = fs::metadata(&file_location).is_ok();
    if !exists {
        match fs::create_dir_all(&file_location) {
            Ok(_) => (),
            Err(e) => {
                tracing::error!("{:?}", e);
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "Internal server error".to_string(),
                });
            }
        }
    };

    // copy blob to the correct location
    match fs::copy(
        format!("/tmp/{}", temp_file_identifier),
        format!("{}/{}", file_location, hash_hex),
    ) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("{:?}", format!("{}/{}", file_location, hash_hex));
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    }

    // remove the temp file
    match fs::remove_file(format!("/tmp/{}", temp_file_identifier)) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    // create metadata blob
    let mut metadata_file = match fs::File::create(format!("{}/{}.json", file_location, hash_hex)) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };
    match metadata_file.write_all(&metadata_body) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    }

    // return the hash
    HttpResponse::Ok().json(UploadBlobResponse { hash: hash_hex })
}
