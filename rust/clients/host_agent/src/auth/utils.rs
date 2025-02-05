use data_encoding::BASE64URL_NOPAD;

// // NB: These should match the names of these files when saved locally upon hpos init
// // (should be the same as those in the `orchestrator_setup` file)
// const JWT_DIR_NAME: &str = "jwt";
// const OPERATOR_JWT_FILE_NAME: &str = "holo_operator";
// const SYS_JWT_FILE_NAME: &str = "sys_account";
// const WORKLOAD_JWT_FILE_NAME: &str = "workload_account";

/// Encode a JSON string into a b64-encoded string
pub fn json_to_base64(json_data: &str) -> Result<String, serde_json::Error> {
    // Parse to ensure it's valid JSON
    let parsed_json: serde_json::Value = serde_json::from_str(json_data)?;
    // Convert JSON back to a compact string
    let json_string = serde_json::to_string(&parsed_json)?;
    // Encode it into b64
    let encoded = BASE64URL_NOPAD.encode(json_string.as_bytes());
    Ok(encoded)
}

