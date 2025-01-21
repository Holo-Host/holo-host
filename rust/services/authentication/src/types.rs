use std::collections::HashMap;

use util_libs::js_stream_service::{CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AuthServiceSubjects {
    StartHandshake,
    HandleHandshakeP1,
    HandleHandshakeP2,
    EndHandshake,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthState {
    Requested,
    ValidatedAgent, //    AddedHostPubkey
    SignedJWT,
    Authenticated
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthStatus {
    pub host_pubkey: String,
    pub status: AuthState
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthHeaders {
    signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthRequestPayload {
    pub hoster_pubkey: String,
    pub email: String,
    pub host_pubkey: String,
    pub nonce: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthResultType {
    Single(Vec<u8>),
    Multiple(Vec<Vec<u8>>)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthResult {
    pub data: AuthResultType,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthApiResult {
    pub status: AuthStatus,
    pub result: AuthResult,
    pub maybe_response_tags: Option<HashMap<String, String>>
}
impl EndpointTraits for AuthApiResult {}
impl CreateTag for AuthApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
