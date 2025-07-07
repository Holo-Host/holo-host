use db_utils::schemas::hoster::Hoster;
use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use textnonce::TextNonce;
use thiserror::Error;

// The workload_sk_role is assigned when the host agent is created during the auth flow.
// NB: This role name *must* match the `ROLE_NAME_WORKLOAD` in the `hub_auth_setup.sh` script file.
pub const WORKLOAD_SK_ROLE: &str = "workload_role";

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
    pub device_id: String,
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
    Callout(String), // stringified `AuthResponseClaim`
    Authorization(AuthJWTResult),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthApiResult {
    pub result: AuthResult,
    // NB: `maybe_response_tags` optionally return endpoint scoped vars to be available for use as a response subject in the Jetstream Service Endpoint Handler
    pub maybe_response_tags: Option<HashMap<String, String>>,
}
// NB: The following Traits make API Service compatible as a JS Service Endpoint
impl EndpointTraits for AuthApiResult {}
impl GetSubjectTags for AuthApiResult {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl GetResponse for AuthApiResult {
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
impl GetHeaderMap for AuthApiResult {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        None
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserEmail {
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct HosterPubkey {
    pub pubkey: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DbValidationData {
    pub jurisdiction: String,
    pub user_info: UserEmail,
    #[serde(rename = "hoster")]
    pub hoster_pubkey: HosterPubkey,
    #[serde(rename = "hoster_record")]
    pub hoster: Hoster,
}

//////////////////////////
// Auth Callout Types
//////////////////////////
// Callout Request Types:
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthGuardToken {
    pub device_id: String,   // host machine id
    pub host_pubkey: String, // host pubkey(nkey)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoster_hc_pubkey: Option<String>, // holochain encoded hoster pubkey
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub nonce: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    host_signature: Vec<u8>, // used to verify the host keypair
}
// NB: Currently there is no way to pass headers in the auth callout.
// Therefore the host_signature is passed within the b64 encoded `AuthGuardToken` token
impl AuthGuardToken {
    pub fn from_args(
        host_pubkey: String,
        device_id: String,
        nonce: TextNonce,
        hc_pubkey: String,
        email: String,
    ) -> Self {
        #![allow(clippy::field_reassign_with_default)]
        let mut auth_guard_token = AuthGuardToken::default();
        auth_guard_token.host_pubkey = host_pubkey;
        auth_guard_token.device_id = device_id;
        auth_guard_token.nonce = nonce.to_string();
        auth_guard_token.hoster_hc_pubkey = Some(hc_pubkey);
        auth_guard_token.email = Some(email);
        auth_guard_token
    }

    pub fn try_add_signature<T>(mut self, sign_handler: T) -> AuthSignResult<Self>
    where
        T: Fn(&[u8]) -> AuthSignResult<String>,
    {
        let payload_bytes = serde_json::to_vec(&self)?;
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
    pub user_auth_token: String, // This is the b64 encoding of the `AuthGuardToken` -- used to validate user
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

// Shared authentication error type that can be used by both service API and clients
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Signature creation failed: {0}")]
    SignatureFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Service error: {0}")]
    ServiceError(String),
}

impl AuthError {
    pub fn signature_failed(msg: &str) -> Self {
        Self::SignatureFailed(msg.to_string())
    }

    pub fn auth_failed(msg: &str) -> Self {
        Self::AuthenticationFailed(msg.to_string())
    }

    pub fn config_error(msg: &str) -> Self {
        Self::ConfigurationError(msg.to_string())
    }

    pub fn service_error(msg: &str) -> Self {
        Self::ServiceError(msg.to_string())
    }
}

pub type AuthSignResult<T> = Result<T, AuthError>;
