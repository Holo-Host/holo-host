/// This function is used to get the API key hash from the api key header and the API key
pub fn hash_apikey(header: String, api_key: String) -> Option<String> {
    if header == "v0" {
        return Some(api_key);
    }
    None
}
