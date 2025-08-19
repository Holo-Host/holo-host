use actix_web::HttpRequest;
use db_utils::schemas::{
    api_key::API_KEY_COLLECTION_NAME, manifest::MANIFEST_COLLECTION_NAME,
    user::USER_COLLECTION_NAME, workload::WORKLOAD_COLLECTION_NAME,
};

pub const API_KEY_HEADER: &str = "x-api-key";
pub const ALL_RESOURCES: [&str; 4] = [
    USER_COLLECTION_NAME,
    WORKLOAD_COLLECTION_NAME,
    API_KEY_COLLECTION_NAME,
    MANIFEST_COLLECTION_NAME,
];

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

pub fn generate_api_key() -> String {
    let key = bson::uuid::Uuid::new().to_string();
    key.replace("-", "")
}
