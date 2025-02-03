use std::collections::HashMap;

use util_libs::js_stream_service::{CreateResponse, CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};

pub const AUTH_SERVICE_SUBJECT: &str = "validate";

// The workload_sk_role is assigned when the host agent is created during the auth flow.
// NB: This role name *must* match the `ROLE_NAME_WORKLOAD` in the `orchestrator_setup.sh` script file.
pub const WORKLOAD_SK_ROLE: &str = "workload-role";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthState {
    Unauthenticated,
    Authenticated,
    Forbidden,
    Error(String)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthGuardPayload {
    pub host_pubkey: String, // nkey
    pub hoster_pubkey: String, // nkey
    pub email: String,
    pub nonce: String
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthRequestPayload {
    pub host_pubkey: String, // nkey
    pub sys_pubkey: Option<String>, // nkey
    pub nonce: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthApiResult {
    pub host_pubkey: String,
    pub status: AuthState,
    pub host_jwt: String,
    pub sys_jwt: String
} 
