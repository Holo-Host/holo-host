use crate::{
    jetstream_client::{get_event_listeners, JsClient},
    types::{JsClientBuilder, JsServiceBuilder, NatsRemoteArgs, PublishInfo},
};
use anyhow::Result;
use mock_utils::test_nats_server::{check_nats_server, TestNatsServer};
use serial_test::serial;
use std::time::Duration;

#[tokio::test]
#[serial]
async fn test_client_initialization() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;

    let client = JsClient::new(JsClientBuilder {
        nats_remote_args: NatsRemoteArgs::try_new(&format!("nats://localhost:{}", server.port))?,
        name: "test_client".to_string(),
        inbox_prefix: "_INBOX".to_string(),
        credentials: None,
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(5)),
        listeners: get_event_listeners(),
    })
    .await
    .expect("Failed to spin up Jetstream Client");

    assert_eq!(client.name, "test_client");

    // Test connection state
    let state = client
        .check_connection()
        .await
        .expect("Failed to get JsClient state...");
    assert!(matches!(state, async_nats::connection::State::Connected));

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_publish_message() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;

    let client = JsClient::new(JsClientBuilder {
        nats_remote_args: NatsRemoteArgs::try_new(&format!("nats://localhost:{}", server.port))?,
        name: "test_client".to_string(),
        inbox_prefix: "_INBOX".to_string(),
        credentials: None,
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(5)),
        listeners: get_event_listeners(),
    })
    .await
    .expect("Failed to spin up Jetstream Client");

    let publish_info = PublishInfo {
        subject: "test.subject".to_string(),
        msg_id: "test_msg_1".to_string(),
        data: b"test message".to_vec(),
        headers: None,
    };

    let result = client.publish(publish_info).await;
    assert!(result.is_ok());

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_add_js_service() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;

    let mut client = JsClient::new(JsClientBuilder {
        nats_remote_args: NatsRemoteArgs::try_new(&format!("nats://localhost:{}", server.port))?,
        name: "test_client".to_string(),
        inbox_prefix: "_INBOX".to_string(),
        credentials: None,
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(5)),
        listeners: get_event_listeners(),
    })
    .await
    .expect("Failed to spin up Jetstream Client");

    let service_params = JsServiceBuilder {
        name: "test_service".to_string(),
        description: "Test Service".to_string(),
        version: "0.0.1".to_string(),
        service_subject: "TEST".to_string(),
    };

    let _ = client.add_js_service(service_params).await.unwrap();

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_client_close() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;

    let client = JsClient::new(JsClientBuilder {
        nats_remote_args: NatsRemoteArgs::try_new(&format!("nats://localhost:{}", server.port))?,
        name: "test_client".to_string(),
        inbox_prefix: "_INBOX".to_string(),
        credentials: None,
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(5)),
        listeners: get_event_listeners(),
    })
    .await
    .expect("Failed to spin up Jetstream Client");

    let result = client.close().await;
    assert!(result.is_ok());

    let _ = server.shutdown().await;
    Ok(())
}
