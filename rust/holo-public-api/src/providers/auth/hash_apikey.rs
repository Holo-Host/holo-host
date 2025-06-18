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
