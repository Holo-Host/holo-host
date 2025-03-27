use std::fs;
use std::io::Write;

use actix_web::{post, HttpMessage, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use blake3;
use crate::providers::{error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(OpenApi)]
#[openapi(
    paths(upload_blob),
    components(schemas(UploadBlobResponse))
)]
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
    let temp_file_identifier = bson::uuid::Uuid::new().to_string();
    let mut hasher = blake3::Hasher::new();
    let mut blob = match fs::File::create(format!("/tmp/{}", temp_file_identifier)) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!("Error creating blob: {:?}", e);
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
            match blob.write_all(&chunk) {
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

    // finalize the hash
    let hash_hex = hasher.finalize().to_hex().to_string();

    // create metadata body
    let metadata_body = bson::doc! {
        "createdAt": bson::DateTime::now().to_string(),
        "updatedAt": bson::DateTime::now().to_string(),
        "userId": auth.sub.clone(),
        "hash": hash_hex.clone(),
    }
    .to_string()
    .into_bytes();

    // create directory for the blob and metadata
    let file_location = format!("./srv/holo-blobstore");
    match fs::create_dir_all(file_location.clone()) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error creating directory: {:?}", e);
        }
    }

    // move blob to the correct location
    match fs::rename(
        format!("/tmp/{}", temp_file_identifier),
        format!("{}/{}", file_location, hash_hex)
    ) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error moving blob to the correct location: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    }

    // create metadata blob
    let mut metadata_file = match fs::File::create(
        format!("{}/{}.json", file_location, hash_hex)
    ) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!("Error creating metadata blob: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };
    match metadata_file.write_all(&metadata_body) {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error writing metadata blob: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    }

    // return the hash
    HttpResponse::Ok().json(UploadBlobResponse {
        hash: hash_hex,
    })
}
