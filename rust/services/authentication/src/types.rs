use util_libs::js_stream_service::{CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthState {
    Requested,
    ValidatedAgent, //    AddedHostPubkey
    SignedJWT,
    Authenticated
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthStatus {
    pub host_id: String,
    pub status: AuthState
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthHeaders {
    signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthRequestPayload {
    pub email: String,
    pub host_id: String,
    pub pubkey: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiResult {
    pub status: AuthStatus,
    pub result: Vec<u8>,
    pub maybe_response_tags: Option<Vec<String>>
}
impl EndpointTraits for ApiResult {}
impl CreateTag for ApiResult {
    fn get_tags(&self) -> Option<Vec<String>> {
        self.maybe_response_tags.clone()
    }
}
