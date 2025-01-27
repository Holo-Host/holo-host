use std::collections::HashMap;

use util_libs::js_stream_service::{CreateResponse, CreateTag, EndpointTraits};
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
pub struct AuthResultData {
    pub inner: HashMap<String,Vec<u8>>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthResult {
    pub status: AuthStatus,
    pub data: AuthResultData,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthApiResult {
    pub result: AuthResult,
    pub maybe_response_tags: Option<HashMap<String, String>>
}
impl EndpointTraits for AuthApiResult {}
impl CreateTag for AuthApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl CreateResponse for AuthApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let r = self.result.clone();
        match serde_json::to_vec(&r) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
