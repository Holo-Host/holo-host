use super::*;
use crate::nats::leaf_server::{JetStreamConfig, LeafNodeRemote, LeafServer, LoggingOptions};
use dotenv::dotenv;
use futures::StreamExt;
use serial_test::serial;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::process::Command;

const TEST_CONFIG_PATH: &str = "./test_configs/";
const HUB_SERVER_CONFIG_PATH: &str = "./test_configs/hub_server.conf";
const TMP_JS_DIR: &str = "./tmp/leaf_store";
const NEW_LEAF_CONFIG_PATH: &str = "./test_configs/leaf_server.conf";

#[tokio::test]
#[serial]
async fn test_leaf_server_run() {
    dotenv().ok();
    env_logger::init();
    if !Path::new(TEST_CONFIG_PATH).exists() {
        fs::create_dir_all(TEST_CONFIG_PATH).expect("Failed to create leaf confg dir");
    };
    if !Path::new(HUB_SERVER_CONFIG_PATH).exists() {
        panic!("Failed to locate the required HUB_SERVER_CONFIG_PATH")
    }
    if Path::new(NEW_LEAF_CONFIG_PATH).exists() {
        fs::remove_file(NEW_LEAF_CONFIG_PATH).expect("Failed to create leaf confg dir");
    };

    if Path::new(TMP_JS_DIR).exists() {
        fs::remove_dir_all(TMP_JS_DIR)
            .expect("Failed to delete already existing tmp jetstream store dir");
    }
    fs::create_dir_all(TMP_JS_DIR).expect("Failed to create leaf confg dir");

    let local_conn_domain = "127.0.0.1";
    let leaf_server_conn_port = 4111;
    let leaf_conn_url = format!("{}:{}", local_conn_domain, leaf_server_conn_port);
    let leaf_server_remote_conn_port = 7422;
    let leaf_server_remote_conn_url = format!(
        "nats://{}:{}",
        local_conn_domain, leaf_server_remote_conn_port
    );
    let hub_conn_port = 4333;
    let hub_conn_url = format!("{}:{}", local_conn_domain, hub_conn_port);

    let store = PathBuf::from_str(TMP_JS_DIR).expect("Failed to convert str into PathBuf");

    let jetstream_config = JetStreamConfig {
        store_dir: store,
        max_memory_store: 1024 * 1024 * 1024, // 1 GB
        max_file_store: 1024 * 1024 * 1024,   // 1 GB
    };

    let logging_options = LoggingOptions {
        debug: true, // NB: This logging is a blocking action, only run in non-prod
        trace: true, // NB: This logging is a blocking action, only run in non-prod
        longtime: false,
    };

    gen_test_agents_for_leaf(&hub_conn_url);

    // Start the Hub Server in a separate thread
    log::info!("Spawning Hub server");
    tokio::spawn(async move {
        // Run the Hub Server with the (prexisting) config
        Command::new("nats-server")
            .args(["-c", HUB_SERVER_CONFIG_PATH])
            .kill_on_drop(true)
            .spawn()
            .expect("Failed to start Hub server");

        log::info!("Hub Server is running...");

        // Push auth updates to hub server
        Command::new("nsc")
            .arg("push -A")
            .output()
            .await
            .expect("Failed to create resolver config file");
    });
    // Wait for Hub server to be ready
    sleep(Duration::from_secs(1)).await;

    // Create a new Leaf Server instance
    let leaf_node_remotes = vec![
        LeafNodeRemote {
            url: leaf_server_remote_conn_url.to_string(),
            credentials: Some(
                PathBuf::from_str(&format!(
                    "{}/{}/SYS/sys.creds",
                    NSC_CREDS_PATH, OPERATOR_NAME
                ))
                .expect("Faield to convert str into PathBuf"),
            ),
            tls: None,
        },
        LeafNodeRemote {
            url: leaf_server_remote_conn_url.to_string(),
            credentials: Some(
                PathBuf::from_str(&format!(
                    "{}/{}/{}/{}.creds",
                    NSC_CREDS_PATH, OPERATOR_NAME, USER_ACCOUNT_NAME, USER_NAME
                ))
                .expect("Faield to convert str into PathBuf"),
            ),
            tls: None,
        },
    ];

    let leaf_server = LeafServer::new(
        Some("test_leaf_server"),
        NEW_LEAF_CONFIG_PATH,
        local_conn_domain,
        leaf_server_conn_port,
        jetstream_config,
        logging_options,
        leaf_node_remotes,
    );

    // Start the Leaf Server in a separate thread
    log::info!("Spawning Leaf Server");
    let leaf_server_clone = leaf_server.clone();
    let leaf_server_task = tokio::spawn(async move {
        leaf_server_clone
            .run()
            .await
            .expect("Failed to run Leaf Server");
    });
    // Wait for Leaf Server to be conn_result.is_ok
    sleep(Duration::from_secs(1)).await;

    // Connect client to the leaf server
    log::info!("Running client connection test");
    let nsc_user_creds_path = format!(
        "{}/{}/{}/{}.creds",
        NSC_CREDS_PATH, OPERATOR_NAME, USER_ACCOUNT_NAME, USER_NAME
    );
    let conn_result = ConnectOptions::new()
        .name("test_client")
        .credentials_file(nsc_user_creds_path)
        .await
        .expect("Failed to get creds file to connect cllient to test nats leaf server.")
        .connect(leaf_conn_url)
        .await;

    assert!(conn_result.is_ok());
    let client = conn_result.expect("Failed to connect to NATS Leaf server");

    // Verify the connection is active
    assert_eq!(
        client.connection_state(),
        async_nats::connection::State::Connected
    );
    log::debug!("Client successfully connected to the Leaf Server.");

    // Test jetstream calls on leaf client:
    let test_stream_name = "test_stream";
    let test_stream_subject = "test.subject";
    let js_context = async_nats::jetstream::new(client.clone());
    let stream = js_context
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: test_stream_name.to_string(),
            subjects: vec![test_stream_subject.to_string()],
            ..Default::default()
        })
        .await
        .expect("Failed to create stream");

    let stream_info = stream.get_info().await.expect("Failed to get stream info");
    assert_eq!(stream_info.config.name, "test_stream");
    assert!(stream_info
        .config
        .subjects
        .contains(&test_stream_subject.to_string()));

    // Test client publishing to js stream
    let test_msg = "Hello, there. From Leaf!";
    js_context
        .publish(test_stream_subject, test_msg.into())
        .await
        .expect("Failed to publish jetstream message.");

    let test_stream_consumer_name = "test_stream_consumer".to_string();
    let consumer = stream
        .get_or_create_consumer(
            &test_stream_consumer_name.to_string(),
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some(test_stream_consumer_name),
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to add consumer");

    let mut messages = consumer
        .messages()
        .await
        .expect("Failed to fetch messages")
        .take(1);

    let msg_option_result = messages.next().await;
    assert!(msg_option_result.is_some());

    let msg_result = msg_option_result.unwrap();
    assert!(msg_result.is_ok());

    let msg = msg_result.expect("No message received");
    assert_eq!(msg.payload, test_msg);

    // Shut down the client
    client
        .drain()
        .await
        .expect("Failed to drain and flush client");

    // Await server task termination
    let _ = leaf_server_task.await;

    // Clean up temporary dir & files
    std::fs::remove_dir_all(JWT_TEST_DIR).expect("Failed to delete jwt test dir");
    std::fs::remove_dir_all(TEST_AUTH_DIR).expect("Failed to delete test auth dir");
    std::fs::remove_dir_all(TMP_JS_DIR).expect("Failed to delete tmp js dir");
    std::fs::remove_dir_all(NSC_CREDS_PATH).expect("Failed to delete nsc creds dir");
    std::fs::remove_file(NEW_LEAF_CONFIG_PATH).expect("Failed to delete config file");
    std::fs::remove_file(RESOLVER_FILE_PATH).expect("Failed to delete config file");
    std::fs::remove_dir_all(LOCAL_DIR).expect("Failed to delete .local dir");
    std::fs::remove_dir_all(TEMP_DIR).expect("Failed to delete tmp dir");
}
