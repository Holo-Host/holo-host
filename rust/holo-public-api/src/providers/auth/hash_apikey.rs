use db_utils::schemas::api_key::{ApiKey, API_KEY_COLLECTION_NAME};

use crate::providers::crud;

/// This function is used to get the API key hash from the api key header and the API key
pub fn hash_apikey(header: String, api_key: String) -> Option<String> {
    match header.as_str() {
        "v0" => Some(api_key),
        "v1" => bcrypt::hash(api_key, bcrypt::DEFAULT_COST)
            .map(|hash| Some(hash.to_string()))
            .unwrap_or(None),
        _ => None,
    }
}

pub async fn compare_and_fetch_apikey(
    header: String,
    api_key: String,
    db: mongodb::Client,
) -> Option<ApiKey> {
    match header.as_str() {
        "v0" => crud::find_one::<ApiKey>(
            db,
            db_utils::schemas::api_key::API_KEY_COLLECTION_NAME.to_string(),
            bson::doc! { "api_key": api_key.clone() },
        )
        .await
        .unwrap_or_default(),
        "v1" => {
            let prefix_length = 6;
            let prefix = api_key.chars().take(prefix_length).collect::<String>();
            compare_apikey_with_prefix(api_key, prefix, db.clone(), |api_key, hash| {
                bcrypt::verify(api_key, hash.clone().as_str()).unwrap_or(false)
            })
            .await
        }
        _ => None,
    }
}

pub async fn compare_apikey_with_prefix(
    api_key: String,
    prefix: String,
    db: mongodb::Client,
    compareer: fn(api_key: String, hash: String) -> bool,
) -> Option<ApiKey> {
    // let prefix = api_key.chars().take(prefix_length).collect::<String>();
    let matching_apikeys = match crud::list::<ApiKey>(
        db,
        API_KEY_COLLECTION_NAME.to_string(),
        Some(bson::doc! {
            "prefix": prefix
        }),
        None,
        None,
        None,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("{}", error);
            return None;
        }
    };

    for apikey_obj in matching_apikeys {
        if compareer(api_key.clone(), apikey_obj.clone().api_key) {
            return Some(apikey_obj.clone());
        }
    }

    None
}
