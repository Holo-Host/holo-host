use sha2::Digest;

/// This function is used to get the API key hash from the api key header and the API key
pub fn hash_apikey(header: String, api_key: String) -> Option<String> {
    match header.as_str() {
        "v0" => Some(api_key),
        "v1" => {
            let mut hasher = sha2::Sha256::new();
            hasher.update(api_key.as_bytes());
            let result = hasher.finalize();
            Some(format!("{:x}", result))
        }
        _ => None,
    }
}
