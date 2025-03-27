use super::TestHttpServer;
use crate::types::nats::HTTP_GW_SUBJECT_NAME;

use futures::StreamExt;
use hyper::StatusCode;
use mock_utils::test_nats_server::TestNatsServer;
use tokio::time::{Duration, sleep};
use uuid::Uuid;
use workload::WORKLOAD_SRV_SUBJ;

#[tokio::test]
async fn test_e2e_gateway_request() -> anyhow::Result<()> {
    // Start both servers
    let nats_server = TestNatsServer::new().await?;
    let test_client = nats_server.connect(&nats_server.port).await?;
    let http_server = TestHttpServer::new().await?;

    // Set up test data
    let coordinator_id = Uuid::new_v4().to_string();
    let response_data = "test response data".as_bytes().to_vec();

    // Set up NATS subscription
    let subject = format!(
        "{}.{}.{}",
        WORKLOAD_SRV_SUBJ, HTTP_GW_SUBJECT_NAME, coordinator_id
    );
    println!(">>>>>>>>>>> subject: {subject}");

    let reply_subject = format!("{subject}.reply");
    let mut subscription = test_client.client.subscribe(reply_subject).await?;

    // Handle NATS messages
    let client_clone = test_client.client.clone();
    let response_data_clone = response_data.clone();
    tokio::spawn(async move {
        while let Some(msg) = subscription.next().await {
            println!(">>>>>>>>>>> msg: {msg:?}");

            if let Some(reply) = msg.reply {
                println!(">>>>>>>>>>> reply: {reply:?}");

                let _ = client_clone
                    .publish(reply, response_data_clone.clone().into())
                    .await;
                break;
            }
        }
    });

    sleep(Duration::from_secs(4)).await;

    // Make HTTP request
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "{}/testDNAhash123/{}/zome1/fn1?payload=zomecallpayload",
            http_server.address(),
            coordinator_id
        ))
        .header("host", "localhost:8000")
        .send()
        .await?;

    println!("response: {response:?}");

    assert_eq!(response.status(), StatusCode::OK);

    // Verify response headers
    assert!(response.headers().contains_key("X-Holo-RequestID"));
    assert!(response.headers().contains_key("X-Holo-ForwarderID"));

    assert_eq!(response.bytes().await?, response_data);

    nats_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_malformed_gateway_request() -> anyhow::Result<()> {
    let http_server = TestHttpServer::new().await?;

    // Make HTTP request to the test server
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/testDNAhash123", http_server.address()))
        .header("host", "localhost:8000")
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response_string = String::from_utf8(response.bytes().await?.into()).unwrap();
    assert!(response_string.contains("Found unrecognized route on Holo Gateway"));

    Ok(())
}
