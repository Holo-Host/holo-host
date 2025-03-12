use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

use crate::providers::{error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(OpenApi)]
#[openapi(
    paths(upload_happ),
    components(schemas(UploadHappResponse))
)]
pub struct OpenApiSpec;

#[derive(Serialize, ToSchema)]
pub struct UploadHappResponse {
    pub happ_public_url: String,
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
    let file_name = bson::uuid::Uuid::new().to_string();

    // Start the multipart upload.
    let upload = match s3_client
        .create_multipart_upload()
        .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
        .bucket(bucket)
        .key(format!("{}/{}", auth.sub, file_name))
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

            // Convert chunk into a ByteStream.
            let body = ByteStream::from(chunk.to_vec());

            // Upload the part.
            let part_resp = match s3_client
                .upload_part()
                .bucket(bucket)
                .key(format!("{}/{}", auth.sub, file_name))
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

            // Collect the ETag and part number.
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
        }
    }

    // Build the completed multipart upload structure.
    let completed_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();

    // Complete the multipart upload.
    let complete_resp = match s3_client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(format!("{}/{}", auth.sub, file_name))
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
    

    // Optionally, you can extract information from complete_resp if needed.
    HttpResponse::Ok().json(UploadHappResponse {
        happ_public_url: format!("https://{}", complete_resp.location.unwrap_or_default()),
    })
}
