use actix_web::HttpRequest;
use bson::{doc, oid::ObjectId};
use db_utils::{
    mongodb::{api::MongoDbAPI, collection::MongoCollection},
    schemas::{
        api_key::{ApiKey, API_KEY_COLLECTION_NAME},
        user::{User, USER_COLLECTION_NAME},
        user_permissions::UserPermission,
    },
};

use super::jwt::{sign_access_token, sign_refresh_token, AccessTokenClaims, RefreshTokenClaims};

const API_KEY_HEADER: &str = "x-api-key";

/// This function is used to get the refresh token version
/// if it cannot locate the refresh token version, it returns 0
pub async fn get_refresh_token_version(db: &mongodb::Client, user_id: String) -> i32 {
    let collection = match MongoCollection::<User>::new(db, "holo", USER_COLLECTION_NAME).await {
        Ok(collection) => collection,
        Err(_err) => {
            return 0;
        }
    };
    let oid = match ObjectId::parse_str(user_id) {
        Ok(oid) => oid,
        Err(_err) => {
            return 0;
        }
    };
    let doc = match collection
        .get_one_from(doc! { "_id": oid, "metadata.is_deleted": false })
        .await
    {
        Ok(doc) => doc,
        Err(_err) => {
            return 0;
        }
    };
    if doc.is_none() {
        return 0;
    }
    let doc = doc.unwrap();
    doc.refresh_token_version
}

/// This function is used to get the user id and permissions from the api key
/// It returns an Option<ApiKey> which contains the user id and permissions
/// If the api key is not found, it returns None
pub async fn get_user_id_and_permissions_from_apikey(
    db: &mongodb::Client,
    api_key_hash: String,
) -> Result<Option<ApiKey>, anyhow::Error> {
    let collection = match MongoCollection::<ApiKey>::new(db, "holo", API_KEY_COLLECTION_NAME).await
    {
        Ok(collection) => collection,
        Err(_err) => {
            print!("Failed to get MongoDB collection");
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    let result = match collection
        .get_one_from(doc! { "api_key": api_key_hash, "metadata.is_deleted": false })
        .await
    {
        Ok(result) => result,
        Err(_err) => {
            print!("Failed to get MongoDB collection 2");
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    if result.is_none() {
        return Ok(None);
    }
    let result = result.unwrap();
    Ok(Some(result))
}

/// This function signs a access and a refresh token
/// and returns them as a tuple
pub fn sign_jwt_tokens(
    jwt_secret: &str,
    user_id: String,
    permissions: Vec<UserPermission>,
    version: i32,
    allow_extending_refresh_token: bool,
    expires_at: usize,
    api_key: Option<String>,
) -> Option<(String, String)> {
    const ACCESS_TOKEN_EXPIRATION: usize = 60 * 5; // 5 minutes
    let access_token = match sign_access_token(
        AccessTokenClaims {
            sub: user_id.clone(),
            permissions: permissions.clone(),
            exp: bson::DateTime::now().to_chrono().timestamp() as usize + ACCESS_TOKEN_EXPIRATION,
        },
        jwt_secret,
    ) {
        Ok(claims) => claims,
        Err(_err) => {
            tracing::error!("failed to sign access token");
            return None;
        }
    };
    let refresh_token = match sign_refresh_token(
        RefreshTokenClaims {
            sub: user_id.clone(),
            exp: expires_at,
            version,
            allow_extending_refresh_token,
            api_key,
        },
        jwt_secret,
    ) {
        Ok(token) => token,
        Err(_err) => {
            tracing::error!("failed to sign refresh token");
            return None;
        }
    };
    Some((access_token, refresh_token))
}

/// This function is used to get the API key hash from the api key header and the API key
pub fn get_apikey_hash(header: String, api_key: String) -> Option<String> {
    if header == "v0" {
        return Some(api_key);
    }
    None
}

/// This function is used to get the API key from the headers
pub fn get_apikey_from_headers(req: &HttpRequest) -> Option<String> {
    match req.headers().get(API_KEY_HEADER) {
        None => None,
        Some(apikey) => match apikey.to_str() {
            Err(_err) => None,
            Ok(api_key) => Some(api_key.to_string()),
        },
    }
}

/// get an api key using mongodb id
pub async fn get_api_key(
    db: &mongodb::Client,
    api_key_id: String,
) -> Result<Option<ApiKey>, anyhow::Error> {
    let collection = match MongoCollection::<ApiKey>::new(db, "holo", API_KEY_COLLECTION_NAME).await
    {
        Ok(collection) => collection,
        Err(_err) => {
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    let oid = match ObjectId::parse_str(api_key_id) {
        Ok(value) => value,
        Err(_err) => return Err(anyhow::anyhow!("Failed to get object id")),
    };
    let result = match collection
        .get_one_from(doc! { "_id": oid, "metadata.is_deleted": false })
        .await
    {
        Ok(result) => result,
        Err(_err) => {
            return Err(anyhow::anyhow!("Failed to get MongoDB collection"));
        }
    };
    if result.is_none() {
        return Ok(None);
    }
    let result = result.unwrap();
    Ok(Some(result))
}
