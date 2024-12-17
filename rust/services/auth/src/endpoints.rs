// use super::types;
use super::nats_client::{self, EventListener};
use super::AUTH_SRV_OWNER_NAME;

use anyhow::{anyhow, Result};
// use async_nats::HeaderValue;
use async_nats::Message;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
// use async_nats::jetstream::{Context};
// use async_nats::jetstream::ErrorCode;
// use async_nats::jetstream::consumer::Consumer;
// use async_nats::jetstream::consumer::PullConsumer;
// use async_nats::jetstream::consumer::pull::Stream;
// // use std::io::Read;
// use tokio::fs::OpenOptions;
// use tokio::{fs::File, io::AsyncWriteExt};
// use tokio::io;
// use futures::future;
// use futures::stream::{self, StreamExt};


// NB: Message { subject, reply, payload, headers, status, description, length }

// const CHUNK_SIZE: usize = 1024; // 1 KB chunks

pub async fn start_handshake() ->  nats_client::AsyncEndpointHandler {
    Arc::new(
        move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
            log::warn!("INCOMING Message for 'AUTH.start_handshake' : {:?}", msg);
            let msg_clone = msg.clone();
            Box::pin(async move {
                // 1. Verify expected data was received
                if msg_clone.headers.is_none() {
                    log::error!("Error: Missing headers. Consumer=authorize_ext_client, Subject='/AUTH/authorize'");
                    // anyhow!(ErrorCode::BAD_REQUEST)
                }

                // let signature = msg_clone.headers.unwrap().get("Signature").unwrap_or(&HeaderValue::new());

                // match  serde_json::from_str::<types::AuthHeaders>(signature.as_str()) {
                //     Ok(r) => {}
                //     Err(e) => {
                //         log::error!("Error: Failed to deserialize headers. Consumer=authorize_ext_client, Subject='/AUTH/authorize'")
                //         // anyhow!(ErrorCode::BAD_REQUEST)
                //     }
                // }

                // match serde_json::from_slice::<types::AuthPayload>(msg.payload.as_ref()) {
                //     Ok(r) => {}
                //     Err(e) => {
                //         log::error!("Error: Failed to deserialize payload. Consumer=authorize_ext_client, Subject='/AUTH/authorize'")
                //         // anyhow!(ErrorCode::BAD_REQUEST)
                //     }
                // }

                // 2. Authenticate the HPOS client(?via email and host id info?)

                // 3. Publish operator and sys account jwts for orchestrator
                // let hub_operator_account = chunk_and_publish().await; // returns to the `save_hub_files` subject
                // let hub_sys_account = chunk_and_publish().await; // returns to the `save_hub_files` subject

                let response = serde_json::to_vec(&"OK")?;
                Ok(response)
            })
        },
    )
}

pub async fn save_hub_auth() ->  nats_client::AsyncEndpointHandler {
    Arc::new(
        move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
            log::warn!("INCOMING Message for 'AUTH.save_hub_auth' : {:?}", msg);
            Box::pin(async move {
                // receive_and_write_file();

                // Respond to endpoint request
                // let response = b"Hello, NATS!".to_vec();
                // Ok(response)
                
                todo!();
            })
        },
    )
}

pub async fn send_user_pubkey() ->  nats_client::AsyncEndpointHandler {
    Arc::new(
        move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
            log::warn!("INCOMING Message for 'AUTH.send_user_pubkey' : {:?}", msg);
            Box::pin(async move {
                // 1. validate nk key...
                // let auth_endpoint_subject =
                // format!("AUTH.{}.file.transfer.JWT-operator", "host_id_placeholder"); // endpoint_subject
            
                // 2. Update the hub nsc with user pubkey
                
                // 3. create signed jwt
            
                // 4. `Ack last request and publish the new jwt to for hpos
            
                // 5. Respond to endpoint request
                // let response = b"Hello, NATS!".to_vec();
                // Ok(response)

                todo!()
            })
        },
    )
}


    // let auth_endpoint_subject = format!("AUTH.{}.file.transfer.JWT-User", host_id); // endpoint_subject
    // let c = js_service
    //     .add_local_consumer(
    //         "save_user_auth", // called from orchestrator (no auth service)
    //         "save_user_auth",
    //         EndpointType::Async(Arc::new(
    //             async |msg: &Message| -> Result<Vec<u8>, anyhow::Error> {
    //                 log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

    //                 receive_and_write_file()

    //                 // 2. Respond to endpoint request
    //                 let response = b"Hello, NATS!".to_vec();
    //                 Ok(response)
    //             },
    //         )),
    //         None,
    //     )
    //     .await?;

    // let c = js_service
    //     .add_local_consumer(
    //         "send_user_file", //  called from orchestrator (no auth service)
    //         "send_user_file",
    //         EndpointType::Async(Arc::new(
    //             async |msg: &Message| -> Result<Vec<u8>, anyhow::Error> {
    //                 log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

    //                 receive_and_write_file()

    //                 // 2. Respond to endpoint request
    //                 let response = b"Hello, NATS!".to_vec();
    //                 Ok(response)
    //             },
    //         )),
    //         None,
    //     )
    //     .await?;

    //     let c = js_service
    //     .add_local_consumer(
    //         "save_user_file", // called from hpos
    //         "end_hub_handshake",
    //         EndpointType::Async(Arc::new(
    //             async |msg: &Message| -> Result<Vec<u8>, anyhow::Error> {
    //                 log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

    //                 receive_and_write_file()

    //                 // 2. Respond to endpoint request
    //                 let response = b"Hello, NATS!".to_vec();
    //                 Ok(response)
    //             },
    //         )),
    //         None,
    //     )
    //     .await?;



// ==================== Helpers ====================

pub fn get_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = |msg: &str, _duration: Duration| {
        log::info!(
            "Successfully published message for {} client: {:?}",
            AUTH_SRV_OWNER_NAME,
            msg
        );
    };
    let failure_handler = |err: &str, _duration: Duration| {
        log::error!(
            "Failed to publish message for {} client: {:?}",
            AUTH_SRV_OWNER_NAME,
            err
        );
    };

    let event_listeners = vec![
        nats_client::on_msg_published_event(published_msg_handler),
        nats_client::on_msg_failed_event(failure_handler),
    ];

    event_listeners
}


// async fn chunk_file_and_publish(js: &Context, subject: &str, file_path: &str) -> io::Result<()> {
//     let mut file = std::fs::File::open(file_path)?;
//     let mut buffer = vec![0; CHUNK_SIZE];
//     let mut chunk_id = 0;

//     while let Ok(bytes_read) = file.read(mut buffer) {
//         if bytes_read == 0 {
//             break;
//         }
//         let chunk_data = &buffer[..bytes_read];
//         js.publish(subject.to_string(), chunk_data.into()).await.unwrap();
//         chunk_id += 1;
//     }

//     // Send an EOF marker
//     js.publish(subject.to_string(), "EOF".into()).await.unwrap();

//     Ok(())
// }

// async fn receive_and_write_file(stream: Stream, consumer: PullConsumer, header_subject: String, output_dir: &str, file_name: &str) -> Result<(), std::io::Error> {
//     let output_path = format!("{}/{}", output_dir, file_name);
//     let mut file = OpenOptions::new().create(true).append(true).open(&output_path)?;

//     let mut messages = consumer
//         .stream()
//         .max_messages_per_batch(100)
//         .max_bytes_per_batch(1024)
//         .heartbeat(std::time::Duration::from_secs(10))
//         .messages()
//         .await?;

//     // while let Some(Ok(message)) = message.next().await {}
//     let file_msgs = messages().take_while(|msg| future::ready(*msg.subject.contains(file_name)));
//     while let Some(Ok(msg)) = file_msgs.next().await {
//         if msg.payload.to_string().contains("EOF") {
//             // if msg.payload == b"EOF" {
//             msg.ack().await?;
//             println!("File transfer complete.");
//             return Ok(());
//         }

//         file.write_all(&msg.payload).await?;
//         file.flush().await?;
//         msg.ack().await?;
//     }

//     Ok(())
// }