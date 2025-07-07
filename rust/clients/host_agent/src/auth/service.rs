/// pub async fn `authorize_host` - Attempts to authorize a host agent with the authentication service.
///
/// This function performs the complete authentication flow:
/// 1. Loads hoster configuration and creates authentication guard payload
/// 2. Establishes authenticated connection to NATS authentication service
/// 3. Creates and signs JWT authentication payload
/// 4. Sends authentication request with signature
/// 5. Handles response and saves new credentials on successful authorization
/// 6. Falls back to unauthenticated inventory reporting on timeout
///
/// The function uses the host agent's keys to sign authentication payloads and,
/// upon successful authentication, saves the received JWT credentials for future use.
///
/// # Arguments
/// * `device_id` - Unique identifier for the device being authenticated
/// * `host_agent_keys` - Keys for the host agent (moved into client config)
/// * `hub_url` - URL of the NATS hub to connect to for authentication
///
/// # Returns
/// * `Ok((Keys, async_nats::Client))` - Updated keys with new credentials and authenticated client
/// * `Err(HostAgentError)` - Authentication error information
///
/// # Behavior
/// - On success: Returns keys updated with new JWT credentials
/// - On timeout: Attempts to publish inventory data and returns unauthenticated client
/// - On denial/error: Returns appropriate error with context
///
use crate::{
    auth::{client::AuthClient, config::HosterConfig, keys::Keys},
    local_cmds::host::errors::{HostAgentError, HostAgentResult},
    local_cmds::host::types::agent_client::{
        ClientType, HostAuthArgs, HostClient, HostClientConfig,
    },
};

use async_nats::{HeaderMap, HeaderName, HeaderValue, RequestErrorKind};
use authentication::{
    types::{AuthGuardToken, AuthJWTPayload, AuthJWTResult, AuthSignResult, AuthState},
    AUTH_SRV_SUBJ, VALIDATE_AUTH_SUBJECT,
};
use hpos_hal::inventory::HoloInventory;
use std::str::FromStr;
use textnonce::TextNonce;

pub async fn authorize_host(
    device_id: &str,
    host_agent_keys: Keys,
    hub_url: &str,
) -> HostAgentResult<(Keys, async_nats::Client)> {
    log::info!("Host Auth Client: Connecting to server...");
    log::trace!(
        "Host Auth Client: Host Agent Keys before authentication request: {:#?}",
        host_agent_keys
    );

    // Fetch Hoster Pubkey and email (from hpos-config file)
    let config = HosterConfig::new().await?;

    // Create Auth Guard toekn (without sig)
    let mut auth_guard_token = AuthGuardToken::from_args(
        host_agent_keys.host_pubkey.clone(),
        device_id.to_string(),
        TextNonce::new(),
        config.hc_pubkey,
        config.email,
    );

    // Sign Auth Guard payload and add into this token
    let signing_function =
        |p: &[u8]| -> AuthSignResult<String> { host_agent_keys.host_sign(p).map_err(|e| e.into()) };
    auth_guard_token = auth_guard_token
        .try_add_signature(signing_function)
        .map_err(|e| HostAgentError::auth_failed(&format!("Signature creation failed: {}", e)))?;

    // Create Host Client Config and start Host Auth Client
    let host_auth_args = HostAuthArgs {
        hub_url: hub_url.to_string(),
        auth_guard_token,
    };
    let host_client_config = HostClientConfig::new(
        device_id,
        host_agent_keys.clone(),
        ClientType::HostAuth(host_auth_args),
    )?;
    let AuthClient {
        client: auth_guard_client,
        ..
    } = AuthClient::start(&host_client_config).await?;

    // Create AuthJWTPayload and send to NATS AuthCallout Service
    let payload = AuthJWTPayload {
        device_id: device_id.to_string(),
        host_pubkey: host_agent_keys.host_pubkey.clone(),
        maybe_sys_pubkey: host_agent_keys.local_sys_pubkey.clone(), // Will be set from keys if available
        nonce: TextNonce::new().to_string(),
    };
    let payload_bytes = serde_json::to_vec(&payload)?;

    let signature = host_agent_keys.host_sign(&payload_bytes)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("X-Signature"),
        HeaderValue::from_str(&signature)?,
    );

    log::debug!(
        "Host Agent Auth Service: about to publish authenication request to {}.{}",
        AUTH_SRV_SUBJ,
        VALIDATE_AUTH_SUBJECT
    );

    // Call NATS AuthCallout Service to Authenticate Host & Hoster
    // NB: When successful, this will authenticate both the host and sys user
    let response_msg = match auth_guard_client
        .request_with_headers(
            format!("{}.{}", AUTH_SRV_SUBJ, VALIDATE_AUTH_SUBJECT),
            headers,
            payload_bytes.into(),
        )
        .await
    {
        Ok(msg) => msg,
        Err(e) => {
            log::error!(
                "Host Agent Auth Service: Authentication request failed: {:#?}",
                e
            );

            // Handle timeout, falling back to the unauthenticated inventory publication
            if let RequestErrorKind::TimedOut = e.kind() {
                let unauthenticated_user_inventory_subject =
                    format!("INVENTORY.{}.unauthenticated", host_agent_keys.host_pubkey);
                let diagnostics = HoloInventory::from_host();
                let inventory_bytes = serde_json::to_vec(&diagnostics)?;

                match auth_guard_client
                    .publish(
                        unauthenticated_user_inventory_subject,
                        inventory_bytes.into(),
                    )
                    .await
                {
                    Ok(_) => {
                        log::warn!("Host Agent Auth Service: Authentication timed out, but inventory published successfully. Returning unauthenticated client.");
                        return Ok((host_agent_keys, auth_guard_client));
                    }
                    Err(publish_err) => {
                        log::error!("Host Agent Auth Service: Failed to publish inventory after timeout: {}", publish_err);
                        return Err(HostAgentError::auth_failed(&format!(
                            "Authentication timed out and inventory publish failed: {}",
                            publish_err
                        )));
                    }
                }
            }
            return Err(e.into());
        }
    };

    // Deserialize the response and handle errors
    let auth_response = serde_json::from_slice::<AuthJWTResult>(&response_msg.payload)?;

    log::info!(
        "Host Agent Auth Service: Received AUTH response: {:#?}",
        auth_response
    );

    // Handle authentication response with explicit state matching
    let updated_keys = match auth_response.status {
        AuthState::Authorized => {
            log::info!("Host Agent Auth Service: Host authentication successful");
            host_agent_keys
                .save_host_creds(auth_response.host_jwt, auth_response.sys_jwt)
                .await?
        }
        AuthState::Authenticated => {
            return Err(HostAgentError::auth_failed("Host authorization is incomplete. The hpos is authenticated but the associated hoster authorization is missing and required"));
        }
        AuthState::Forbidden => {
            return Err(HostAgentError::auth_failed(
                "Host authentication denied by server",
            ));
        }
        AuthState::Unauthenticated => {
            return Err(HostAgentError::auth_failed(
                "Host authentication is still unauthenticated - try again later",
            ));
        }
        AuthState::Error(msg) => {
            return Err(HostAgentError::auth_failed(&format!(
                "Host authentication error: {}",
                msg
            )));
        }
    };

    Ok((updated_keys, auth_guard_client))
}
