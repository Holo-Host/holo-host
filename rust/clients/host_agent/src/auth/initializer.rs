/*
 This client is associated with the:
- ADMIN account
- noauth user

...once this the host and hoster are validated, this client should close and the hpos manager should spin up.

// This client is responsible for:
1. generating new key / re-using the user key from provided file
2. calling the auth service to:
  - verify host/hoster via `auth/start_hub_handshake` call
  - get hub operator jwt and hub sys account jwt via `auth/start_hub_handshake`
  - send "nkey" version of pubkey as file to hub via via `auth/end_hub_handshake`
  - get user jwt from hub via `auth/save_`
3. create user creds file with file path
4. instantiate the leaf server via the leaf-server struct/service
*/

use anyhow::Result;
// use auth::AUTH_SRV_NAME;
use std::time::Duration;
use util_libs::nats_client::{self, Client as NatClient, EventListener};

const HOST_INIT_CLIENT_NAME: &str = "Host Initializer";
const HOST_INIT_CLIENT_INBOX_PREFIX: &str = "_host_init_inbox";

pub async fn run() -> Result<String, async_nats::Error> {
    log::info!("Host Initializer Client: Connecting to server...");
    // 1. Connect to Nats server
    let nats_url = nats_client::get_nats_url();
    let event_listeners = get_init_host_event_listeners();

    let init_host_client = nats_client::DefaultClient::new(nats_client::NewDefaultClientParams {
        nats_url,
        name: HOST_INIT_CLIENT_NAME.to_string(),
        inbox_prefix: HOST_INIT_CLIENT_INBOX_PREFIX.to_string(),
        credentials_path: None,
        opts: vec![nats_client::with_event_listeners(event_listeners)],
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(5)),
    })
    .await?;

    // Discover the server Node ID via INFO response
    let server_node_id = init_host_client.client.server_info().server_id;
    log::trace!(
        "Host Initializer Client: Retrieved Node ID: {}",
        server_node_id
    );

    // Publish a message with the Node ID as part of the subject
    let publish_options = nats_client::PublishOptions {
        subject: format!("HPOS.init.{}", server_node_id),
        msg_id: format!("hpos_init_mid_{}", rand::random::<u8>()),
        data: b"Host Initializer Connected!".to_vec(),
    };

    match init_host_client
        .publish_with_retry(&publish_options, 3)
        .await
    {
        Ok(_r) => {
            log::trace!("Host Initializer Client: Node ID published.");
        }
        Err(_e) => {}
    };

    // Call auth service and preform handshake
    // let auth_service = init_host_client.get_stream(AUTH_SRV_NAME).await?;
    // i. call `save_hub_auth`
    // ii. call `start_hub_handshake`
    // iii. THEN (once get resp from start_handshake) `send_user_pubkey`
    // iv. call`end_hub_handshake`
    // v. call save_user_file``

    // 5. Create creds
    let user_creds_path = "/path/to/host/user.creds".to_string();

    // 6. Close and drain internal buffer before exiting to make sure all messages are sent
    init_host_client.close().await?;

    Ok(user_creds_path)
}

pub fn get_init_host_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = |msg: &str, _duration: Duration| {
        log::info!(
            "Successfully published message for {} client: {:?}",
            HOST_INIT_CLIENT_NAME,
            msg
        );
    };
    let failure_handler = |err: &str, _duration: Duration| {
        log::error!(
            "Failed to publish message for {} client: {:?}",
            HOST_INIT_CLIENT_NAME,
            err
        );
    };

    let event_listeners = vec![
        nats_client::on_msg_published_event(published_msg_handler),
        nats_client::on_msg_failed_event(failure_handler),
    ];

    event_listeners
}
