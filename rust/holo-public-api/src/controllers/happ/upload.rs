use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use aws_sdk_s3::Client as S3Client;
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
    pub url: String,
    pub hash: String,
    pub metadata: String,
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
    mut payload: Multipart,
    s3_client: web::Data<S3Client>,
    config: web::Data<AppConfig>,
) -> impl Responder {
    let bucket = "holo-public-api-dev-storage";
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

    // Start the multipart upload.
    let upload = match s3_client
        .create_multipart_upload()
        .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
        .bucket(bucket)
        .key(format!("{}/{}/file", auth.sub, file_identifier))
        .content_type("application/octet-stream")
        .send()
        .await
    {
        Ok(upload) => upload,
        Err(e) => {
            tracing::error!("Error creating multipart upload: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    // We'll collect the completed parts (with part number and ETag).
    let mut completed_parts: Vec<CompletedPart> = Vec::new();
    let mut part_number = 1;

    // Accumulator for building parts.
    let mut part_buffer: Vec<u8> = Vec::new();

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
            part_buffer.extend_from_slice(&chunk);

            // If the accumulator has reached at least MIN_PART_SIZE, upload it as a part.
            if part_buffer.len() >= MIN_PART_SIZE {
                let body = ByteStream::from(part_buffer.clone());
                let part_resp = match s3_client
                    .upload_part()
                    .bucket(bucket)
                    .key(format!("{}/{}/file", auth.sub, file_identifier))
                    .body(body)
                    .part_number(part_number)
                    .upload_id(upload.upload_id().unwrap())
                    .send()
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        tracing::error!("Error uploading part {}: {:?}", part_number, e);
                        return HttpResponse::InternalServerError().json(ErrorResponse {
                            message: "Internal server error".to_string(),
                        });
                    }
                };

                if let Some(e_tag) = part_resp.e_tag {
                    completed_parts.push(
                        CompletedPart::builder()
                            .set_part_number(Some(part_number))
                            .set_e_tag(Some(e_tag))
                            .build(),
                    );
                } else {
                    tracing::error!("Missing ETag for part {}", part_number);
                    return HttpResponse::InternalServerError().json(ErrorResponse {
                        message: "Internal server error".to_string(),
                    });
                }
                part_number += 1;
                part_buffer.clear();
            }
        }
    }

    // Upload any remaining data as the final part.
    if !part_buffer.is_empty() {
        let body = ByteStream::from(part_buffer.clone());
        let part_resp = match s3_client
            .upload_part()
            .bucket(bucket)
            .key(format!("{}/{}/file", auth.sub, file_identifier))
            .body(body)
            .part_number(part_number)
            .upload_id(upload.upload_id().unwrap())
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("Error uploading final part {}: {:?}", part_number, e);
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "Internal server error".to_string(),
                });
            }
        };

        if let Some(e_tag) = part_resp.e_tag {
            completed_parts.push(
                CompletedPart::builder()
                    .set_part_number(Some(part_number))
                    .set_e_tag(Some(e_tag))
                    .build(),
            );
        } else {
            tracing::error!("Missing ETag for final part {}", part_number);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    } else if completed_parts.is_empty() {
        // If no parts were uploaded, the file might be smaller than the multipart threshold.
        // You might choose to use a simple put_object in this case.
        tracing::error!("No file data was received.");
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "No file data provided.".to_string(),
        });
    }

    // Build the completed multipart upload structure.
    let completed_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();

    // Complete the multipart upload.
    let _ = match s3_client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(format!("{}/{}/file", auth.sub, file_identifier))
        .upload_id(upload.upload_id().unwrap())
        .multipart_upload(completed_upload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Error completing multipart upload: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    let hash_hex = hasher.finalize().to_hex().to_string();

    let metadata_body = ByteStream::from(
        bson::doc! {
            "createdAt": bson::DateTime::now(),
            "updatedAt": bson::DateTime::now(),
            "fileIdentifier": file_identifier.clone(),
            "userId": auth.sub.clone(),
            "hash": hash_hex.clone(),
        }
        .to_string()
        .into_bytes(),
    );

    // Upload metadata.
    let _ = match s3_client
        .put_object()
        .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
        .bucket(bucket)
        .key(format!("{}/{}/metadata.json", auth.sub, file_identifier))
        .body(metadata_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Error uploading metadata: {:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    let endpoint = config.object_storage_endpoint.clone();
    let metadata_url = format!(
        "https://{}.{}/{}/{}/metadata.json",
        bucket, endpoint, auth.sub, file_identifier
    );
    let url = format!(
        "https://{}.{}/{}/{}/file",
        bucket, endpoint, auth.sub, file_identifier
    );
    
    HttpResponse::Ok().json(UploadHappResponse {
        url,
        metadata: metadata_url,
        hash: hash_hex,
    })
}
