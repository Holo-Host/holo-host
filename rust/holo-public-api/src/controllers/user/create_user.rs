use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas::{
    self,
    user::{User, UserPubKey, UserRole, USER_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use serde::{Deserialize, Serialize};
use strum::Display;
use utoipa::{openapi, OpenApi, PartialSchema, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(Serialize, Deserialize, Display)]
#[serde(rename_all = "snake_case")]
pub enum PublicKeyRoleInfo {
    Developer,
    Hoster,
}

impl PartialSchema for PublicKeyRoleInfo {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        let schema = openapi::schema::Object::builder()
            .schema_type(openapi::schema::SchemaType::Type(
                openapi::schema::Type::Object,
            ))
            .title(Some("Public Key Role Info".to_string()))
            .examples(vec![
                PublicKeyRoleInfo::Developer.to_string(),
                PublicKeyRoleInfo::Hoster.to_string(),
            ])
            .build();

        openapi::RefOr::T(openapi::schema::Schema::Object(schema))
    }
}
impl ToSchema for PublicKeyRoleInfo {}

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
    #[schema(example = "john.doe@example.com")]
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

    let result = match providers::crud::create::<User>(
        db.get_ref().clone(),
        USER_COLLECTION_NAME.to_string(),
        User {
            _id: ObjectId::new(),
            metadata: db_utils::schemas::metadata::Metadata::default(),
            permissions: payload.permissions.clone(),
            roles: payload.roles.clone(),
            refresh_token_version: 0,
            public_keys: payload
                .public_keys
                .iter()
                .map(|pub_key| match pub_key.role {
                    PublicKeyRoleInfo::Hoster => UserPubKey::Hoster(pub_key.public_key.to_string()),
                    PublicKeyRoleInfo::Developer => {
                        UserPubKey::Developer(pub_key.public_key.to_string())
                    }
                })
                .collect(),
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
        schemas::user_info::USER_INFO_COLLECTION_NAME.to_string(),
        schemas::user_info::UserInfo {
            _id: ObjectId::new(),
            metadata: db_utils::schemas::metadata::Metadata::default(),
            owner: result,
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
