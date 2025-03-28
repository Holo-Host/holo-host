use super::TestHttpServer;
use hyper::StatusCode;
use std::collections::HashMap;

#[tokio::test]
async fn test_health_check() -> anyhow::Result<()> {
    let http_server: TestHttpServer = TestHttpServer::new().await?;

    // Make HTTP request to the test server
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health", http_server.address()))
        .header("host", "localhost:8000")
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let data = response.text().await?;
    assert_eq!(data, "Ok");

    Ok(())
}

#[tokio::test]
async fn test_unsupported_method() -> anyhow::Result<()> {
    let http_server: TestHttpServer = TestHttpServer::new().await?;
    let valid_addr = format!("{}/", http_server.address());
    let body: HashMap<String, String> = HashMap::new();

    // Make HTTP request to the test server
    let client = reqwest::Client::new();
    let response = client.post(valid_addr).json(&body).send().await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response_string = String::from_utf8(response.bytes().await?.into()).unwrap();
    assert!(response_string.contains("Found unrecognized route on Holo Gateway"));

    Ok(())
}

#[tokio::test]
async fn test_invalid_request() -> anyhow::Result<()> {
    let http_server: TestHttpServer = TestHttpServer::new().await?;

    // Make HTTP request to the test server
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/invalid-url", http_server.address()))
        .header("host", "localhost:8000")
        .send()
        .await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response_string = String::from_utf8(response.bytes().await?.into()).unwrap();
    assert!(response_string.contains("Found unrecognized route on Holo Gateway"));

    Ok(())
}
