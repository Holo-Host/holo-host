use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas::{
    self,
    developer::DEVELOPER_COLLECTION_NAME,
    workload::{HappBinaryFormat, WorkloadManifestHolochainDhtV1, WORKLOAD_COLLECTION_NAME},
};
use url::Url;
use utoipa::OpenApi;

use crate::{
    controllers::workload::workload_dto::{CreateWorkloadDto, WorkloadDto},
    providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims},
};

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
    request_body = CreateWorkloadDto,
    responses(
        (status = 200, body = WorkloadDto)
    )
)]
#[post("/v1/workload")]
pub async fn create_workload(
    req: HttpRequest,
    payload: web::Json<CreateWorkloadDto>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let payload = payload.into_inner();
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();
    let user_id = match ObjectId::parse_str(claims.sub.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::Forbidden().json(ErrorResponse {
                message: "Permission denied".to_string(),
            });
        }
    };

    let developer = match providers::crud::find_one::<schemas::developer::Developer>(
        db.as_ref().clone(),
        DEVELOPER_COLLECTION_NAME.to_string(),
        bson::doc! {
            "user_id": user_id,
        },
    )
    .await
    {
        Ok(developer) => developer,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::Forbidden().json(ErrorResponse {
                message: "Permission denied".to_string(),
            });
        }
    };
    if developer.is_none() {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }
    let developer = developer.unwrap();

    // Verify permissions for creating a workload
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::developer::DEVELOPER_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Create,
            owner: developer.user_id.to_hex(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let workload_id = match providers::crud::create(
        db.as_ref().clone(),
        WORKLOAD_COLLECTION_NAME.to_string(),
        schemas::workload::Workload {
            _id: ObjectId::new(),
            metadata: schemas::metadata::Metadata::default(),
            assigned_developer: developer._id.unwrap(),
            assigned_hosts: vec![],
            version: "0.0.1".to_string(),
            min_hosts: payload.properties.instances.unwrap_or(1),
            system_specs: schemas::workload::SystemSpecs {
                capacity: schemas::workload::Capacity {
                    drive: 1, // Default drive space
                    cores: 1, // Default cores
                },
                avg_network_speed: 0, // Default network speed
                avg_uptime: 0.0,      // Default uptime requirement
            },
            status: schemas::workload::WorkloadStatus {
                id: None,
                desired: schemas::workload::WorkloadState::Reported,
                actual: schemas::workload::WorkloadState::Reported,
                payload: schemas::workload::WorkloadStatePayload::None,
            },
            manifest: schemas::workload::WorkloadManifest::HolochainDhtV1(Box::new(
                WorkloadManifestHolochainDhtV1 {
                    happ_binary: HappBinaryFormat::HappBinaryBlake3Hash(
                        payload.template.blake3_hash.clone(),
                    ),
                    stun_server_urls: payload.template.stun_server_urls.map(|urls| {
                        urls.iter()
                            .map(|url| Url::parse(url).unwrap())
                            .collect::<Vec<Url>>()
                    }),
                    holochain_feature_flags: payload.template.holochain_feature_flags.clone(),
                    holochain_version: payload.template.holochain_version.clone(),

                    // properties
                    network_seed: payload.properties.network_seed.clone(),
                    memproof: payload.properties.memproof.clone(),
                    bootstrap_server_url: match payload.properties.bootstrap_server_url.clone() {
                        Some(url) => match Url::parse(&url) {
                            Ok(parsed_url) => Some(parsed_url),
                            Err(_) => {
                                return HttpResponse::BadRequest().json(ErrorResponse {
                                    message: "Invalid URL for bootstrap server".to_string(),
                                });
                            }
                        },
                        None => None,
                    },
                    signal_server_url: match payload.properties.signal_server_url.clone() {
                        Some(url) => match Url::parse(&url) {
                            Ok(parsed_url) => Some(parsed_url),
                            Err(_) => {
                                return HttpResponse::BadRequest().json(ErrorResponse {
                                    message: "Invalid URL for signal server".to_string(),
                                });
                            }
                        },
                        None => None,
                    },
                    http_gw_enable: payload.properties.http_gw_enable,
                    http_gw_allowed_fns: payload.properties.http_gw_allowed_fns.clone(),
                },
            )),
        },
    )
    .await
    {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to create workload".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(WorkloadDto {
        id: workload_id.to_hex(),
        properties: payload.properties.clone(),
        status: schemas::workload::WorkloadState::Reported,
    })
}
