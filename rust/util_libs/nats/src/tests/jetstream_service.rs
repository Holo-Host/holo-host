use crate::{
    jetstream_service::JsStreamService,
    tests::{
        test_nats_server::{check_nats_server, TestClientResponse, TestNatsServer},
        LocalTestResponse,
    },
    types::{ConsumerBuilder, EndpointType, ResponseSubjectsGenerator},
};
use anyhow::Result;
use futures::StreamExt;
use mock_utils::service_test_response::TestResponse;
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
#[serial]
async fn test_service_initialization() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;
    let TestClientResponse { client: _, js } = server.connect(&server.port).await?;

    let service = JsStreamService::new(
        js,
        "test_service",
        "Test Service Description",
        "0.0.1",
        "TEST",
    )
    .await
    .expect("Failed to spin up Jetstream Service");

    let info = service.get_service_info();
    assert_eq!(info.name, "test_service");
    assert_eq!(info.version, "0.0.1");
    assert_eq!(info.service_subject, "TEST");

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_add_consumer() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;
    let TestClientResponse { client: _, js } = server.connect(&server.port).await?;

    let service = JsStreamService::new(
        js,
        "test_service",
        "Test Service Description",
        "0.0.1",
        "TEST",
    )
    .await
    .expect("Failed to spin up Jetstream Service");

    // Create a sync endpoint handler
    let handler = EndpointType::Sync(Arc::new(|_msg| {
        Ok(LocalTestResponse(TestResponse {
            message: "test response".to_string(),
        }))
    }));

    let consumer_builder = ConsumerBuilder {
        name: "test_consumer".to_string(),
        subject: "endpoint".to_string(),
        handler,
        response_subject_fn: None,
    };

    let _consumer = service
        .add_consumer(consumer_builder)
        .await
        .expect("Failed to add consumer");

    // Verify consumer was created
    let info = service.get_consumer_stream_info("test_consumer").await?;
    assert!(info.is_some());

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_adding_async_consumer() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;
    let TestClientResponse { client: _, js } = server.connect(&server.port).await?;

    let service = JsStreamService::new(
        js.clone(),
        "test_service",
        "Test Service Description",
        "0.0.1",
        "TEST",
    )
    .await
    .expect("Failed to spin up Jetstream Service");

    // Create an async endpoint handler
    let handler = EndpointType::Async(Arc::new(|msg| {
        Box::pin(async move {
            Ok(LocalTestResponse(TestResponse {
                message: format!("async response for {:?}", msg),
            }))
        })
    }));

    let consumer_builder = ConsumerBuilder {
        name: "async_consumer".to_string(),
        subject: "async_endpoint".to_string(),
        handler,
        response_subject_fn: None,
    };

    let _consumer = service
        .add_consumer(consumer_builder)
        .await
        .expect("Failed to add consumer.");

    // Verify consumer was created
    let info = service.get_consumer_stream_info("async_consumer").await?;
    assert!(info.is_some());

    let _ = server.shutdown().await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_consumer_message_handling() -> Result<()> {
    if !check_nats_server() {
        log::debug!("Skipping test: nats-server not available");
        return Ok(());
    }

    let server = TestNatsServer::new().await?;
    let TestClientResponse { client, js } = server.connect(&server.port).await?;

    let service = JsStreamService::new(
        js.clone(),
        "test_service",
        "Test Service Description",
        "0.0.1",
        "TEST",
    )
    .await
    .expect("Failed to spin up Jetstream Service");

    // Create payload that will be published and consumed by endpoint
    let published_payload = TestResponse {
        message: "Incoming test message".to_string(),
    };

    // Create a sync endpoint handler
    let handler = EndpointType::Sync(Arc::new(|msg| {
        let test_str_payload = std::str::from_utf8(&msg.payload).expect("Invalid UTF-8");
        println!(" >> test_str_payload {}", test_str_payload);
        let test_payload = serde_json::from_str::<TestResponse>(test_str_payload)
            .expect("Failed to convert str to TestResponse");
        assert_eq!(test_payload.message, "Incoming test message".to_string());
        Ok(LocalTestResponse(TestResponse {
            message: "This is my outgoing test response".to_string(),
        }))
    }));

    fn response_handler() -> ResponseSubjectsGenerator {
        Arc::new(move |_: HashMap<String, String>| -> Vec<String> { vec!["response".to_string()] })
    }

    let consumer_builder = ConsumerBuilder {
        name: "test_consumer".to_string(),
        subject: "endpoint".to_string(),
        handler,
        response_subject_fn: Some(response_handler()),
    };

    let _consumer = service
        .add_consumer(consumer_builder)
        .await
        .expect("Failed to add consumer.");

    // Spawn the subcription to the response subject
    let s = client.subscribe("TEST.response".to_string()).await;
    assert!(s.is_ok());
    let mut subscriber = s.expect("Failed to create subscriber.");
    subscriber.unsubscribe_after(1).await?;

    tokio::spawn(async move {
        let msg_option_result = subscriber.next().await;
        assert!(msg_option_result.is_some());

        let msg = msg_option_result.unwrap();
        let test_str_payload = std::str::from_utf8(&msg.payload).expect("Invalid UTF-8");
        let test_payload = serde_json::from_str::<TestResponse>(test_str_payload)
            .expect("Failed to convert str to TestResponse");
        assert_eq!(
            test_payload.message,
            "This is my outgoing test response".to_string()
        );

        let _ = server.shutdown().await;
    });

    // Publish the test message
    js.publish(
        "TEST.endpoint".to_string(),
        serde_json::to_vec(&published_payload)?.into(),
    )
    .await
    .expect("Failed to publish message on Jetstream Service");

    // Wait a bit for message processing
    sleep(Duration::from_secs(3)).await;
    Ok(())
}
