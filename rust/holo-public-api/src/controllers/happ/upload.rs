use std::fs;
use std::io::Write;

use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use blake3;

use crate::providers::config::AppConfig;
use crate::providers::{error_response::ErrorResponse, jwt::AccessTokenClaims};

const MIN_PART_SIZE: usize = 5 * 1024 * 1024; // 5 MB

#[derive(OpenApi)]
#[openapi(
    paths(upload_happ),
    components(schemas(UploadHappResponse))
)]
pub struct OpenApiSpec;

#[derive(Serialize, ToSchema)]
pub struct UploadHappResponse {
    pub file_identifier: String,
    pub hash: String,
}

#[derive(Serialize, ToSchema)]
struct FileUploadRequest {
    #[schema(value_type = String, format = "binary")]
    file: Vec<u8>,
}

#[utoipa::path(
    post,
    path = "/protected/v1/happ/upload",
    tag = "Happ",
    summary = "Upload HAPP",
    description = "Upload HAPP",
    security(
        ("Bearer" = [])
    ),
    request_body(
        content = FileUploadRequest,
        content_type = "multipart/form-data",
    ),
    responses(
        (status = 200, body = UploadHappResponse)
    )
)]
#[post("/v1/happ/upload")]
pub async fn upload_happ(
    req: HttpRequest,
    mut payload: Multipart
) -> impl Responder {
    let ext = req.extensions_mut();
    let auth = ext.get::<AccessTokenClaims>();
    if auth.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let auth = auth.unwrap();
    let file_identifier = bson::uuid::Uuid::new().to_string();
    let mut hasher = blake3::Hasher::new();
    fs::create_dir_all(format!("tmp/{}", file_identifier)).unwrap();
    let mut file = match fs::File::create(format!("tmp/{}/file.happ", file_identifier)) {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("Error creating file: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(e) => {
                tracing::error!("Error processing multipart field: {:?}", e);
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
                    tracing::error!("Error reading chunk: {:?}", e);
                    return HttpResponse::BadRequest().json(ErrorResponse {
                        message: "Invalid request".to_string(),
                    });
                }
            };

            // Update hash and append to our accumulator.
            hasher.update(&chunk);
            match file.write_all(&chunk) {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error writing chunk: {:?}", e);
                    return HttpResponse::InternalServerError().json(ErrorResponse {
                        message: "Internal server error".to_string(),
                    });
                }
            }
        }
    }

    let hash_hex = hasher.finalize().to_hex().to_string();

    let metadata_body = bson::doc! {
        "createdAt": bson::DateTime::now().to_string(),
        "updatedAt": bson::DateTime::now().to_string(),
        "fileIdentifier": file_identifier.clone(),
        "userId": auth.sub.clone(),
        "hash": hash_hex.clone(),
    }
    .to_string()
    .into_bytes();

    let mut metadata_file = match fs::File::create(
        format!("tmp/{}/metadata.json", file_identifier)
    ) {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("Error creating metadata file: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };
    metadata_file.write_all(&metadata_body).unwrap();

    HttpResponse::Ok().json(UploadHappResponse {
        file_identifier,
        hash: hash_hex,
    })
}
