use super::types;
use anyhow::{anyhow, Result};
use base32::decode as base32Decode;
use base32::Alphabet;
use data_encoding::{BASE32HEX_NOPAD, BASE64URL_NOPAD};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use nkeys::KeyPair;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Arc;
use std::time::SystemTime;
use util_libs::nats::types::ServiceError;

pub fn handle_internal_err(err_msg: &str) -> ServiceError {
    log::error!("{}", err_msg);
    ServiceError::Internal(err_msg.to_string())
}

pub async fn write_file(data: Vec<u8>, output_dir: &str, file_name: &str) -> Result<String> {
    let output_path = format!("{}/{}", output_dir, file_name);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)?;

    file.write_all(&data)?;
    file.flush()?;
    Ok(output_path)
}

/// Decode a Base64-encoded string back into a JSON string
pub fn base64_to_data<T>(base64_data: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let decoded_bytes = BASE64URL_NOPAD.decode(base64_data.as_bytes())?;
    let json_string = String::from_utf8(decoded_bytes)?;
    let parsed_json: T = serde_json::from_str(&json_string)?;
    Ok(parsed_json)
}

pub fn hash_claim(claims_str: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(claims_str);
    let claims_hash = hasher.finalize();
    claims_hash.as_slice().into()
}

// Convert claims to JWT/Token
pub fn encode_jwt(claims_str: &str, signing_kp: &Arc<KeyPair>) -> Result<String> {
    const JWT_HEADER: &str = r#"{"typ":"JWT","alg":"ed25519-nkey"}"#;
    let b64_header: String = BASE64URL_NOPAD.encode(JWT_HEADER.as_bytes());
    let b64_body = BASE64URL_NOPAD.encode(claims_str.as_bytes());
    let jwt_half = format!("{b64_header}.{b64_body}");
    let sig = signing_kp.sign(jwt_half.as_bytes())?;
    let b64_sig = BASE64URL_NOPAD.encode(&sig);
    Ok(format!("{jwt_half}.{b64_sig}"))
}

/// Convert token into the
pub fn decode_jwt<T>(token: &str, auth_signing_account_pubkey: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    // Decode and replace custom `ed25519-nkey` to `EdDSA`
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid JWT format"));
    }

    // Decode base64 JWT header and fix the algorithm field
    let header_json = BASE64URL_NOPAD.decode(parts[0].as_bytes())?;
    let mut header: Value = serde_json::from_slice(&header_json).expect("failed to create header");

    // Manually fix the algorithm name
    if let Some(alg) = header.get_mut("alg") {
        if alg == "ed25519-nkey" {
            *alg = serde_json::Value::String("EdDSA".to_string());
        }
    }
    let modified_header = BASE64URL_NOPAD.encode(&serde_json::to_vec(&header)?);
    let part_1_json = BASE64URL_NOPAD.decode(parts[1].as_bytes())?;
    let mut part_1: Value = serde_json::from_slice(&part_1_json)?;
    if part_1.get("exp").is_none() {
        let one_week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
        let one_week_from_now = SystemTime::now() + one_week;
        let expires_at: i64 = one_week_from_now
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs()
            .try_into()?;

        let mut b: types::UserClaim = serde_json::from_value(part_1)?;
        b.generic_claim_data.expires_at = Some(expires_at);
        part_1 = serde_json::to_value(b)?;
    }
    let modified_part_1 = BASE64URL_NOPAD.encode(&serde_json::to_vec(&part_1)?);
    let modified_token = format!("{}.{}.{}", modified_header, modified_part_1, parts[2]);

    // Decode from Base32 to raw bytes using Rfc4648 (compatible with NATS keys)
    let public_key_bytes = base32Decode(
        Alphabet::Rfc4648 { padding: false },
        auth_signing_account_pubkey,
    )
    .expect("Failed to convert public key to bytes");

    // Use the decoded key to create a DecodingKey
    let decoding_key = DecodingKey::from_ed_der(&public_key_bytes);

    // Validate the token with the correct algorithm
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.insecure_disable_signature_validation();
    validation.validate_aud = false; // Disable audience validation

    let token_data = decode::<T>(&modified_token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

pub fn generate_auth_response_claim(
    auth_signing_account_keypair: Arc<KeyPair>,
    auth_signing_account_pubkey: String,
    auth_root_account_pubkey: String,
    permissions: types::Permissions,
    auth_request_claim: types::NatsAuthorizationRequestClaim,
) -> Result<types::AuthResponseClaim> {
    let now = SystemTime::now();
    let issued_at = now
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        .try_into()?;
    let one_week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let one_week_from_now = now + one_week;
    let expires_at: i64 = one_week_from_now
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        .try_into()?;
    let inner_generic_data = types::NatsGenericData {
        claim_type: "user".to_string(),
        tags: vec![],
        version: 2,
    };
    let user_claim_data = types::UserClaimData {
        permissions,
        generic_data: inner_generic_data,
        issuer_account: Some(auth_root_account_pubkey.clone()), // must be the root account pubkey or the issuer account that signs the claim AND must be listed "allowed-account"
    };
    let inner_nats_claim = types::ClaimData {
        issuer: auth_signing_account_pubkey.clone(), // Must be the pubkey of the keypair that signs the claim
        subcriber: auth_request_claim.auth_request.user_nkey.clone(),
        issued_at,
        audience: None, // Inner claim should have no `audience` when using the operator-auth mode
        expires_at: Some(expires_at),
        not_before: None,
        name: Some("allowed_auth_user".to_string()),
        jwt_id: None,
    };
    let mut user_claim = types::UserClaim {
        generic_claim_data: inner_nats_claim,
        user_claim_data,
    };

    let mut user_claim_str = serde_json::to_string(&user_claim)?;
    let hashed_user_claim_bytes = hash_claim(&user_claim_str);
    user_claim.generic_claim_data.jwt_id = Some(BASE32HEX_NOPAD.encode(&hashed_user_claim_bytes));
    user_claim_str = serde_json::to_string(&user_claim)?;

    let user_token = encode_jwt(&user_claim_str, &auth_signing_account_keypair)?;
    let outer_nats_claim = types::ClaimData {
        issuer: auth_root_account_pubkey.clone(), // Must be the pubkey of the keypair that signs the claim
        subcriber: auth_request_claim.auth_request.user_nkey.clone(),
        issued_at,
        audience: Some(auth_request_claim.auth_request.server_id.id),
        expires_at: None, // Some(expires_at),
        not_before: None,
        name: None,
        jwt_id: None,
    };
    let outer_generic_data = types::NatsGenericData {
        claim_type: "authorization_response".to_string(),
        tags: vec![],
        version: 2,
    };
    let auth_response = types::AuthGuardResponse {
        generic_data: outer_generic_data,
        user_jwt: Some(user_token),
        issuer_account: None,
        error: None,
    };
    let mut auth_response_claim = types::AuthResponseClaim {
        generic_claim_data: outer_nats_claim,
        auth_response,
    };

    let claim_str = serde_json::to_string(&auth_response_claim)?;
    let hashed_claim_bytes = hash_claim(&claim_str);
    auth_response_claim.generic_claim_data.jwt_id =
        Some(BASE32HEX_NOPAD.encode(&hashed_claim_bytes));

    Ok(auth_response_claim)
}
