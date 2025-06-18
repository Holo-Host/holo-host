// use super::{TestHttpServer, TestNatsServer}; // create_test_request
// use crate::types::{http::ForwardedHTTPRequest, nats::HTTP_GW_SUBJECT_NAME};
// use futures::StreamExt;
// use hyper::{Method, StatusCode};
// use tokio::time::{Duration, sleep};
// use uuid::Uuid;
// use workload::WORKLOAD_SRV_SUBJ;

// #[tokio::test]
// async fn test_e2e_request() -> anyhow::Result<()> {
//     // Start both servers
//     let nats_server = TestNatsServer::new().await?;
//     let test_client = nats_server.connect(&nats_server.port).await?;
//     let http_server = TestHttpServer::new().await?;

//     // Set up test data
//     let coordinator_id = Uuid::new_v4().to_string();
//     let response_data = "test response data".as_bytes().to_vec();

//     // Set up NATS subscription
//     let subject = format!(
//         "{}.{}.{}",
//         WORKLOAD_SRV_SUBJ, HTTP_GW_SUBJECT_NAME, coordinator_id
//     );
//     let mut subscription = test_client.client.subscribe(subject).await?;

//     // Handle NATS messages
//     let client_clone = test_client.client.clone();
//     let response_data_clone = response_data.clone();
//     tokio::spawn(async move {
//         while let Some(msg) = subscription.next().await {
//             println!("msg: {msg:?}");

//             if let Some(reply) = msg.reply {
//                 println!("reply: {reply:?}");

//                 let _ = client_clone
//                     .publish(reply, response_data_clone.clone().into())
//                     .await;
//                 break;
//             }
//         }
//     });

//     sleep(Duration::from_secs(3)).await;

//     // Make HTTP request
//     let client = reqwest::Client::new();
//     let response = client
//         .get(format!(
//             "{}/testDNAhash123/{}/zome1/fn1?payload=test",
//             http_server.address(),
//             coordinator_id
//         ))
//         .header("Host", "localhost:8000")
//         .send()
//         .await?;

//     println!("response: {response:?}");

//     assert!(response.status().is_success());
//     assert_eq!(response.bytes().await?, response_data);

//     nats_server.shutdown().await?;
//     Ok(())
// }

// #[tokio::test]
// async fn test_gateway_malformed_holochain_request() -> anyhow::Result<()> {
//     let http_server = TestHttpServer::new().await?;

//     // Make HTTP request to the test server
//     let client = reqwest::Client::new();
//     let response = client
//         .get(format!("{}/testDNAhash123", http_server.address()))
//         .header("Host", "localhost:8000")
//         .send()
//         .await?;

//     assert_eq!(response.status(), StatusCode::NOT_FOUND);
//     let response_string = String::from_utf8(response.bytes().await?.into()).unwrap();
//     assert!(response_string.contains("Found unrecognized route on Holo Gateway"));

//     Ok(())
// }

// // #[tokio::test]
// // async fn test_forwarded_request_creation() -> anyhow::Result<()> {
// //     let node_id = "test-node";
// //     let test_header_value = "test-value";

// //     let req = create_test_request(
// //         Method::GET,
// //         "/test",
// //         Some(vec![("X-Test", test_header_value)]),
// //     );

// //     let forwarded = ForwardedHTTPRequest::from_hyper(&req, node_id);

// //     // Check required headers
// //     assert!(forwarded.headers.contains_key("X-Holo-RequestID"));
// //     assert_eq!(
// //         forwarded
// //             .headers
// //             .get("X-Holo-ForwarderID")
// //             .map(|v| String::from_utf8(v.clone()).unwrap()),
// //         Some(node_id.to_string())
// //     );

// //     // Check custom header was preserved
// //     assert_eq!(
// //         forwarded
// //             .headers
// //             .get("X-Test")
// //             .map(|v| String::from_utf8(v.clone()).unwrap()),
// //         Some(test_header_value.to_string())
// //     );

// //     Ok(())
// // }
