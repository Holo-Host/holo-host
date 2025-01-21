/*
Endpoints & Managed Subjects:
    - save_hub_jwts: AUTH.<host_pubkey>.handle_handshake_p1
    - save_user_jwt: AUTH.<host_pubkey>.end_hub_handshake
*/

use super::{AuthServiceApi, types, utils};
use anyhow::Result;
use async_nats::Message;
use types::{AuthApiResult, AuthResult};
use core::option::Option::None;
use std::collections::HashMap;
use std::sync::Arc;
use util_libs::nats_js_client::ServiceError;

#[derive(Debug, Clone, Default)]
pub struct HostAuthApi {}

impl AuthServiceApi for HostAuthApi {}

impl HostAuthApi {
    pub async fn save_hub_jwts(&self, msg: Arc<Message>) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.<host_pubkey>.handle_handshake_p1' : {:?}", msg);

        // utils::receive_and_write_file();

        // // Generate resolver file and create resolver file
        // let resolver_path = utils::get_resolver_path();
        // Command::new("nsc")
        //     .arg("generate")
        //     .arg("config")
        //     .arg("--nats-resolver")
        //     .arg("sys-account SYS")
        //     .arg("--force")
        //     .arg(format!("--config-file {}", resolver_path))
        //     .output()
        //     .expect("Failed to create resolver config file");

        // // Push auth updates to hub server
        // Command::new("nsc")
        //     .arg("push -A")
        //     .output()
        //     .expect("Failed to create resolver config file");

        // Prepare to send over user pubkey(to trigger the user jwt gen on hub)
        let user_nkey_path = utils::get_file_path_buf("user_jwt_path");
        let user_nkey: Vec<u8> = std::fs::read(user_nkey_path).map_err(|e| ServiceError::Internal(e.to_string()))?;
        let host_pubkey = serde_json::to_string(&user_nkey).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());
        
        // Respond to endpoint request
        Ok(AuthApiResult {
            status: types::AuthStatus { 
                host_pubkey: host_pubkey.clone(),
                status: types::AuthState::Requested
            },
            result: AuthResult {
                data: types::AuthResultType::Single(user_nkey)
            },
            maybe_response_tags: Some(tag_map) // used to inject as tag in response subject
        })
    }

    pub async fn save_user_jwt(
        &self,
        msg: Arc<Message>,
        _output_dir: &str,
    ) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.<host_pubkey>.end_handshake' : {:?}", msg);

        // Generate user jwt file
        // utils::receive_and_write_file(msg, output_dir, file_name).await?;
        
        // Generate user creds file
        // let _user_creds_path = utils::generate_creds_file();

        // 2. Respond to endpoint request
        Ok(AuthApiResult {
            status: types::AuthStatus { 
                host_pubkey: "host_id_placeholder".to_string(),
                status: types::AuthState::Authenticated
            },
            result: AuthResult {
                data: types::AuthResultType::Single(b"Hello, NATS!".to_vec())
            },
            maybe_response_tags: None
        })
    }
}
