/*
Endpoints & Managed Subjects:
    - save_hub_jwts: AUTH.<host_pubkey>.handle_handshake_p1
    - save_user_jwt: AUTH.<host_pubkey>.end_hub_handshake
*/

use super::{AuthServiceApi, types, utils};
use utils::handle_internal_err;
use anyhow::Result;
use async_nats::Message;
use types::{AuthApiResult, AuthResult};
use core::option::Option::None;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use util_libs::nats_js_client::ServiceError;

#[derive(Debug, Clone, Default)]
pub struct HostAuthApi {}

impl AuthServiceApi for HostAuthApi {}

impl HostAuthApi {
    pub async fn save_hub_jwts(
        &self,
        msg: Arc<Message>,
        output_dir: &str
    ) -> Result<AuthApiResult, ServiceError> {
        let msg_subject = &msg.subject.clone().into_string(); // AUTH.<host_pubkey>.handle_handshake_p1
        log::trace!("Incoming message for '{}'", msg_subject);

        // 1. Verify expected payload was received
        let message_payload = Self::convert_msg_to_type::<AuthResult>(msg.clone())?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let operator_jwt_bytes = message_payload.data.inner.get("operator_jwt").ok_or_else(|| {
            let err_msg = format!("Error: Failed to find operator jwt in message payload. Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;

        let sys_account_jwt_bytes = message_payload.data.inner.get("sys_account_jwt").ok_or_else(|| {
            let err_msg = format!("Error: Failed to find sys jwt in message payload. Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;
        
        let workload_account_jwt_bytes = message_payload.data.inner.get("workload_account_jwt").ok_or_else(|| {
            let err_msg = format!("Error: Failed to find sys jwt in message payload. Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;

        // 2. Save operator_jwt, sys_account_jwt, and workload_account_jwt local to hosting agent
        let operator_jwt_file = utils::receive_and_write_file(operator_jwt_bytes.to_owned(), output_dir, "operator.jwt").await.map_err(|e| {
            let err_msg = format!("Failed to save operator jwt. Subject='{}' Error={}.", msg_subject, e);
            handle_internal_err(&err_msg)
        })?;

        let sys_jwt_file = utils::receive_and_write_file(sys_account_jwt_bytes.to_owned(), output_dir, "account_sys.jwt").await.map_err(|e| {
            let err_msg = format!("Failed to save sys jwt. Subject='{}' Error={}.", msg_subject, e);
            handle_internal_err(&err_msg)
        })?;

        let workload_jwt_file = utils::receive_and_write_file(workload_account_jwt_bytes.to_owned(), output_dir, "account_sys.jwt").await.map_err(|e| {
            let err_msg = format!("Failed to save sys jwt. Subject='{}' Error={}.", msg_subject, e);
            handle_internal_err(&err_msg)
        })?;

        Command::new("nsc")
            .arg(format!("add operator -u {} --force", operator_jwt_file))
            .output()
            .expect("Failed to add operator with provided operator jwt file");

        Command::new("nsc")
            .arg(format!("add import account --file {}", sys_jwt_file))
            .output()
            .expect("Failed to add sys with provided sys jwt file");

        Command::new("nsc")
            .arg(format!("add import account --file {}", workload_jwt_file))
            .output()
            .expect("Failed to add workload account with provided workload jwt file");

        // Command::new("nsc")
        //     .arg(format!("generate nkey -o --store > operator_sk.nk"))
        //     .output()
        //     .expect("Failed to add new operator signing key on hosting agent");

        let host_sys_user_file_name = format!("{}/user_sys_host_{}.nk", output_dir, message_payload.status.host_pubkey);
        Command::new("nsc")
            .arg(format!("generate nkey -u --store > {}", host_sys_user_file_name))
            .output()
            .expect("Failed to add new sys user key on hosting agent");

        // 3. Prepare to send over user pubkey(to trigger the user jwt gen on hub)
        let sys_user_nkey_path = utils::get_file_path_buf(&host_sys_user_file_name);
        let sys_user_nkey: Vec<u8> = std::fs::read(sys_user_nkey_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let host_user_file_name = format!("{}/user_host_{}.nk", output_dir, message_payload.status.host_pubkey);
        let host_user_nkey_path = utils::get_file_path_buf(&host_user_file_name);
        let host_user_nkey: Vec<u8> = std::fs::read(host_user_nkey_path).map_err(|e| ServiceError::Internal(e.to_string()))?;
    
        // let host_pubkey = serde_json::to_string(&user_nkey).map_err(|e| ServiceError::Internal(e.to_string()))?;
        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), message_payload.status.host_pubkey.clone());

        let mut result_hash_map: HashMap<String, Vec<u8>> = HashMap::new();
        result_hash_map.insert("sys_user_nkey".to_string(), sys_user_nkey);
        result_hash_map.insert("host_user_nkey".to_string(), host_user_nkey);
        
        // 4. Respond to endpoint request
        Ok(AuthApiResult {
            result: AuthResult {
                status: types::AuthStatus { 
                    host_pubkey: message_payload.status.host_pubkey,
                    status: types::AuthState::Requested
                },
                data: types::AuthResultData { inner: result_hash_map }
            },
            maybe_response_tags: Some(tag_map) // used to inject as tag in response subject
        })
    }

    pub async fn save_user_jwt(
        &self,
        msg: Arc<Message>,
        output_dir: &str,
    ) -> Result<AuthApiResult, ServiceError> {
        let msg_subject = &msg.subject.clone().into_string(); // AUTH.<host_pubkey>.end_handshake
        log::trace!("Incoming message for '{}'", msg_subject);

        // 1. Verify expected payload was received
        let message_payload = Self::convert_msg_to_type::<AuthResult>(msg.clone())?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let host_sys_user_jwt_bytes = message_payload.data.inner.get("host_sys_user_jwt").ok_or_else(|| {
            let err_msg = format!("Error: . Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;

        let host_user_jwt_bytes = message_payload.data.inner.get("host_user_jwt").ok_or_else(|| {
            let err_msg = format!("Error: Failed to find sys jwt in message payload. Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;

        // 2. Save user_jwt and sys_jwt local to hosting agent
        utils::receive_and_write_file(host_sys_user_jwt_bytes.to_owned(), output_dir, "operator.jwt").await.map_err(|e| {
            let err_msg = format!("Failed to save operator jwt. Subject='{}' Error={}.", msg_subject, e);
            handle_internal_err(&err_msg)
        })?;

        utils::receive_and_write_file(host_user_jwt_bytes.to_owned(), output_dir, "account_sys.jwt").await.map_err(|e| {
            let err_msg = format!("Failed to save sys jwt. Subject='{}' Error={}.", msg_subject, e);
            handle_internal_err(&err_msg)
        })?;

        let host_user_log = Command::new("nsc") 
            .arg(format!("describe user -a WORKLOAD -n user_host_{} --json", message_payload.status.host_pubkey))
            .output()
            .expect("Failed to add user with provided keys");    

        log::debug!("HOST USER JWT: {:?}", host_user_log);

        // 3. Respond to endpoint request
        Ok(AuthApiResult {
            result: AuthResult {
                status: types::AuthStatus { 
                    host_pubkey: message_payload.status.host_pubkey,
                    status: types::AuthState::Authenticated
                },
                data: types::AuthResultData { inner: HashMap::new() }
            },
            maybe_response_tags: None
        })
    }
}
