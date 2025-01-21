use std::process::Command;

pub fn _get_host_user_pubkey_path() -> String {
    std::env::var("HOST_USER_PUBKEY").unwrap_or_else(|_| "./host_user.nk".to_string())
}

pub fn _generate_creds_file() -> String {
    let user_creds_path = "/path/to/host/user.creds".to_string();
    Command::new("nsc")
        .arg(format!("... > {}", user_creds_path))
        .output()
        .expect("Failed to add user with provided keys");

    "placeholder_user.creds".to_string()
}
