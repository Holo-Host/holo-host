use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use util_libs::js_stream_service::{CreateResponse, CreateTag, EndpointTraits};
use data_encoding::BASE64URL_NOPAD;

pub const AUTH_CALLOUT_SUBJECT: &str = "$SYS.REQ.USER.AUTH";
pub const AUTHORIZE_SUBJECT: &str = "validate";

// The workload_sk_role is assigned when the host agent is created during the auth flow.
// NB: This role name *must* match the `ROLE_NAME_WORKLOAD` in the `orchestrator_setup.sh` script file.
pub const WORKLOAD_SK_ROLE: &str = "workload-role";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthState {
    Unauthenticated, // step 0
    Authenticated,   // step 1
    Authorized,      // step 2
    Forbidden,       // failure to auth
    Error(String),   // internal error
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthErrorPayload {
    pub service_info: async_nats::service::Info,
    pub group: String,
    pub endpoint: String,
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthJWTPayload {
    pub host_pubkey: String,              // nkey
    pub maybe_sys_pubkey: Option<String>, // optional nkey
    pub nonce: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthJWTResult {
    pub status: AuthState,
    pub host_pubkey: String,
    pub host_jwt: String,
    pub sys_jwt: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AuthResult {
    Callout(String), // stringifiedAuthResponseClaim
    Authorization(AuthJWTResult),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthApiResult {
    pub result: AuthResult,
    // NB: `maybe_response_tags` optionally return endpoint scoped vars to be available for use as a response subject in JS Service Endpoint handler
    pub maybe_response_tags: Option<HashMap<String, String>>,
}
// NB: The following Traits make API Service compatible as a JS Service Endpoint
impl EndpointTraits for AuthApiResult {}
impl CreateTag for AuthApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl CreateResponse for AuthApiResult {
    fn get_response(&self) -> bytes::Bytes {
        match self.clone().result {
            AuthResult::Authorization(r) => match serde_json::to_vec(&r) {
                Ok(r) => r.into(),
                Err(e) => e.to_string().into(),
            },
            AuthResult::Callout(token) => token.clone().into_bytes().into(),
        }
    }
}

//////////////////////////
// Auth Callout Types
//////////////////////////
// Callout Request Types:
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthGuardPayload {
    pub host_pubkey: String, // nkey pubkey
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoster_hc_pubkey: Option<String>, // holochain encoded hoster pubkey
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub nonce: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    host_signature: Vec<u8>, // used to verify the host keypair
}
// NB: Currently there is no way to pass headers in the auth callout.
// Therefore the host_signature is passed within the b64 encoded `AuthGuardPayload` token
impl AuthGuardPayload {
    pub fn try_add_signature<T>(mut self, sign_handler: T) -> Result<Self>
    where
        T: Fn(&[u8]) -> Result<String>,
    {
        let payload_bytes = serde_json::to_vec(&self)?;
        println!("Going to sign payload_bytes={:?}", payload_bytes);
        let signature = sign_handler(&payload_bytes)?;
        self.host_signature = signature.as_bytes().to_vec();
        Ok(self)
    }

    pub fn without_signature(mut self) -> Self {
        self.host_signature = vec![];
        self
    }

    pub fn get_host_signature(&self) -> Vec<u8> {
        self.host_signature.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NatsAuthorizationRequestClaim {
    #[serde(flatten)]
    pub generic_claim_data: ClaimData,
    #[serde(rename = "nats")]
    pub auth_request: NatsAuthorizationRequest,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NatsAuthorizationRequest {
    pub server_id: NatsServerId,
    pub user_nkey: String,
    pub client_info: NatsClientInfo,
    pub connect_opts: ConnectOptions,
    pub r#type: String, // should be authorization_request
    pub version: u8,    // should be 2
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NatsServerId {
    pub name: String,    // Server name
    pub host: String,    // Server host address
    pub id: String,      // Server connection ID
    pub version: String, // Version of server (current stable = 2.10.22)
    pub cluster: String, // Server cluster name
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NatsClientInfo {
    pub host: String,     // client host address
    pub id: u64,          // client connection ID (I think...)
    pub user: String,     // the user pubkey (the passed-in key)
    pub name_tag: String, // The user pubkey name
    pub kind: String,     // should be "Client"
    pub nonce: String,
    pub r#type: String, // should be "nats"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ConnectOptions {
    #[serde(rename = "auth_token")]
    pub user_auth_token: String, // This is the b64 encoding of the `AuthGuardPayload` -- used to validate user
    #[serde(rename = "jwt")]
    pub user_jwt: String, // This is the jwt string of the `UserClaim`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<u16>,
}

// Callout Response Types:
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthResponseClaim {
    #[serde(flatten)]
    pub generic_claim_data: ClaimData,
    #[serde(rename = "nats")]
    pub auth_response: AuthGuardResponse,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ClaimData {
    #[serde(rename = "iat")]
    pub issued_at: i64, // Issued At (Unix timestamp)
    #[serde(rename = "iss")]
    pub issuer: String, // Issuer -- head account (from which any signing keys were created)
    #[serde(default, rename = "aud", skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>, // Audience for whom the token is intended
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "exp", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>, // Expiry (Optional, Unix timestamp)
    #[serde(default, rename = "jti", skip_serializing_if = "Option::is_none")]
    pub jwt_id: Option<String>, // Base32 hash of the claims
    #[serde(default, rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>, // Issued At (Unix timestamp)
    #[serde(default, rename = "sub")]
    pub subcriber: String, // Public key of the account or user to which the JWT is being issued
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct NatsGenericData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(rename = "type")]
    pub claim_type: String, // should be "user"
    pub version: u8, // should be 2
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct AuthGuardResponse {
    #[serde(flatten)]
    pub generic_data: NatsGenericData,
    #[serde(default, rename = "jwt", skip_serializing_if = "Option::is_none")]
    pub user_jwt: Option<String>, // This is the jwt string of the `UserClaim`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_account: Option<String>, // Issuer Account === the signing nkey. Should set when the claim is issued by a signing key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UserClaim {
    #[serde(flatten)]
    pub generic_claim_data: ClaimData,
    #[serde(rename = "nats")]
    pub user_claim_data: UserClaimData,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UserClaimData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_account: Option<String>,
    #[serde(flatten)]
    pub permissions: Permissions,
    #[serde(flatten)]
    pub generic_data: NatsGenericData,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Permissions {
    #[serde(rename = "pub")]
    pub publish: PermissionLimits,
    #[serde(rename = "sub")]
    pub subscribe: PermissionLimits,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PermissionLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny: Option<Vec<String>>,
}
