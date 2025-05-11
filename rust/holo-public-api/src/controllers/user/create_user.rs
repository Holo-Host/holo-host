use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas::{
    self,
    developer::{Developer, DEVELOPER_COLLECTION_NAME},
    hoster::Hoster,
    user::{RoleInfo, User, UserRole, USER_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicKeyRoleInfo {
    Developer,
    Hoster,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct PublicKeyWithRole {
    /// the public key of the user
    public_key: String,

    /// the role of the user
    role: PublicKeyRoleInfo,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserInfo {
    /// the email of the user, this can be used as a login flow
    #[schema(example = "john.doe@email.com")]
    email: String,

    /// the given names of the user, this can be used as a login flow
    #[schema(example = "John")]
    given_names: String,

    /// the family name of the user, this can be used as a login flow
    #[schema(example = "Doe")]
    family_name: String,

    /// the jurisdiction of the user, this is used to determine the user's permissions
    #[schema(example = json!(db_utils::schemas::jurisdiction::Jurisdiction::default()))]
    geographic_jurisdiction: db_utils::schemas::jurisdiction::Jurisdiction,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    /// user info
    user_info: UserInfo,

    /// additional permissions to give the user
    #[schema(example = json!([]))]
    permissions: Vec<UserPermission>,

    /// roles to assign to the user
    #[schema(example = json!([]))]
    roles: Vec<UserRole>,

    /// public keys to assign to the user
    #[schema(example = json!([]))]
    public_keys: Vec<PublicKeyWithRole>,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateUserResponse {
    /// id of the created user
    id: String,
}

#[derive(OpenApi)]
#[openapi(paths(create_user))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/user",
    tag = "User",
    summary = "Create user",
    description = "Requires 'user.Create' permission, This endpoint is reserved for internal use only",
    request_body = CreateUserRequest,
    responses(
        (status = 200, body = CreateUserResponse)
    )
)]
#[post("/v1/user")]
pub async fn create_user(
    req: HttpRequest,
    payload: web::Json<CreateUserRequest>,
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
        claims,
        vec![UserPermission {
            resource: USER_COLLECTION_NAME.to_string(),
            action: PermissionAction::Create,
            owner: "all".to_string(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Forbidden".to_string(),
        });
    }

    let user_id = ObjectId::new();

    let mut developer_role: Option<RoleInfo> = None;
    let mut hoster_role: Option<RoleInfo> = None;
    for pubkey_obj in payload.public_keys.iter() {
        match pubkey_obj.role {
            PublicKeyRoleInfo::Developer => {
                let result = match providers::crud::create::<Developer>(
                    db.get_ref().clone(),
                    DEVELOPER_COLLECTION_NAME.to_string(),
                    Developer {
                        _id: None,
                        metadata: db_utils::schemas::metadata::Metadata::default(),
                        user_id,
                        active_workloads: vec![],
                    },
                )
                .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        tracing::error!("{:?}", error);
                        return HttpResponse::InternalServerError().json(ErrorResponse {
                            message: "internal server error".to_string(),
                        });
                    }
                };
                developer_role = Some(RoleInfo {
                    collection_id: result,
                    pubkey: pubkey_obj.public_key.clone(),
                });
            }
            PublicKeyRoleInfo::Hoster => {
                let result = match providers::crud::create::<Hoster>(
                    db.get_ref().clone(),
                    USER_COLLECTION_NAME.to_string(),
                    Hoster {
                        _id: None,
                        metadata: db_utils::schemas::metadata::Metadata::default(),
                        user_id,
                        assigned_hosts: vec![],
                    },
                )
                .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        tracing::error!("{:?}", error);
                        return HttpResponse::InternalServerError().json(ErrorResponse {
                            message: "internal server error".to_string(),
                        });
                    }
                };
                hoster_role = Some(RoleInfo {
                    collection_id: result,
                    pubkey: pubkey_obj.public_key.clone(),
                });
            }
        }
    }

    let result = match providers::crud::create::<User>(
        db.get_ref().clone(),
        USER_COLLECTION_NAME.to_string(),
        User {
            _id: Some(user_id),
            metadata: db_utils::schemas::metadata::Metadata::default(),
            permissions: payload.permissions.clone(),
            roles: payload.roles.clone(),
            refresh_token_version: 0,
            developer: developer_role,
            hoster: hoster_role,
            jurisdiction: "".to_string(),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };
    match providers::crud::create::<schemas::user_info::UserInfo>(
        db.get_ref().clone(),
        USER_COLLECTION_NAME.to_string(),
        schemas::user_info::UserInfo {
            _id: None,
            metadata: db_utils::schemas::metadata::Metadata::default(),
            user_id: result.clone(),
            email: payload.user_info.email.clone(),
            given_names: payload.user_info.given_names.clone(),
            family_name: payload.user_info.family_name.clone(),
            geographic_jurisdiction: payload.user_info.geographic_jurisdiction.clone(),
        },
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(CreateUserResponse {
        id: result.to_hex(),
    })
}
