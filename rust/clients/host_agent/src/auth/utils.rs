use data_encoding::BASE64URL_NOPAD;

/// Encode a json string into a b64 string
pub fn json_to_base64(json_data: &str) -> Result<String, serde_json::Error> {
    let parsed_json: serde_json::Value = serde_json::from_str(json_data)?;
    let json_string = serde_json::to_string(&parsed_json)?;
    let encoded = BASE64URL_NOPAD.encode(json_string.as_bytes());
    Ok(encoded)
}
