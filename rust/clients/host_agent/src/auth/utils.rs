use std::{path::PathBuf, process::Command};
use util_libs::nats_js_client::{ServiceError, get_file_path_buf};

use crate::keys;

// NB: These should match the names of these files when saved locally upon hpos init
// (should be the same as those in the `orchestrator_setup` file)
const JWT_DIR_NAME: &str = "jwt";
const OPERATOR_JWT_FILE_NAME: &str = "holo_operator";
const SYS_JWT_FILE_NAME: &str = "sys_account";
const WORKLOAD_JWT_FILE_NAME: &str = "workload_account";

/// Encode a JSON string into a b64-encoded string
fn json_to_base64(json_data: &str) -> Result<String, serde_json::Error> {
    // Parse to ensure it's valid JSON
    let parsed_json: serde_json::Value = serde_json::from_str(json_data)?;
    // Convert JSON back to a compact string
    let json_string = serde_json::to_string(&parsed_json)?;
    // Encode it into b64
    let encoded = general_purpose::STANDARD.encode(json_string);
    Ok(encoded)
}

pub async fn save_host_creds(
    mut host_agent_keys: keys::Keys,
    host_user_jwt: String,
    host_sys_user_jwt: String
) -> Result<keys::Keys, ServiceError> {
    //  Save user jwt and sys jwt local to hosting agent
    utils::write_file(host_user_jwt.as_bytes(), output_dir, "host.jwt").await.map_err(|e| {
        let err_msg = format!("Failed to save operator jwt. Error={}.", e);
        handle_internal_err(&err_msg)
    })?;
    utils::write_file(host_sys_user_jwt.as_bytes(), output_dir, "host_sys.jwt").await.map_err(|e| {
        let err_msg = format!("Failed to save sys jwt. Error={}.", e);
        handle_internal_err(&err_msg)
    })?;

    // Save user creds and sys creds local to hosting agent
    let host_creds_file_name = "host.creds";
    Command::new("nsc")
        .arg(format!("generate creds --name user_host_{} --account {} > {}", host_pubkey, "WORKLOAD", host_creds_file_name))
        .output()
        .expect("Failed to add new operator signing key on hosting agent");
    
    let host_sys_creds_file_name = "host_sys.creds";
    Command::new("nsc")
        .arg(format!("generate creds --name user_host_{} --account {} > {}", host_sys_pubkey, "SYS", host_sys_creds_file_name))
        .output()
        .expect("Failed to add new operator signing key on hosting agent");

    host_agent_keys = host_agent_keys.add_creds_paths(utils::get_file_path_buf(host_creds_file_name), utils::get_file_path_buf(host_sys_creds_file_name));

    Ok(host_agent_keys)
}
