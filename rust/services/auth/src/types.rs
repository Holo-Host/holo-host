use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthHeaders {
    signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuthPayload {
    email: String,
    host_id: String,
    pubkey: String,
}
