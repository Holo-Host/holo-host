use anyhow::Result;
use async_nats::service::endpoint::Endpoint;
use async_nats::service::{endpoint, Group, ServiceExt};
use async_nats::{Client, Message};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

#[derive(Debug, EnumIter, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum Version {
    V1,
    V2,
    V3,
}

// NB: Currently defaults to Version::V1
// TODO: Update to `try_from`
impl From<String> for Version {
    fn from(value: String) -> Self {
        let mut endpoint_version = Version::V1;
        for version in Version::iter() {
            if version.to_string() == value {
                endpoint_version = version
            }
        }
        endpoint_version
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceInfo {
    name: String,
    version: String,
    endpoints: Vec<String>,
}

type EndpointHandler = Arc<dyn Fn(&Message) -> Result<Vec<u8>, anyhow::Error> + Send + Sync>;

type AsyncEndpointHandler = Arc<
    dyn Fn(&Message) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub enum EndpointType {
    Sync(EndpointHandler),
    Async(AsyncEndpointHandler),
}

#[derive(Clone)]
struct GroupExt {
    group: Arc<RwLock<Group>>,
    endpoints: Arc<RwLock<HashMap<String, EndpointType>>>,
}

impl std::fmt::Debug for MicroService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsumerExt")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("log_prefix", &self.log_prefix)
            .finish()
    }
}

#[derive(Clone)]
pub struct MicroService {
    name: String,
    version: String,
    log_prefix: String,
    subject_group: Arc<Group>,
    endpoints_by_version: Arc<Mutex<HashMap<Version, GroupExt>>>,
}

impl MicroService {
    /// Inits microservice
    pub async fn new(
        client: Client,
        name: &str,
        subject: &str,
        description: &str,
        version: &str,
    ) -> Result<Self, async_nats::Error> {
        let service = Arc::new(
            client
                .service_builder()
                .description(description)
                .stats_handler(MicroService::handle_stats)
                .start(name, version)
                .await?,
        );

        let service_info = service.info().await;
        let log_prefix = format!("MS-LOG::{}::{}: ", name, service_info.id);

        let subject_group = Arc::new(Arc::clone(&service).group(subject));
        let endpoints_by_version = MicroService::add_endpoint_versions(
            Arc::new(Mutex::new(HashMap::new())),
            Arc::clone(&subject_group),
            vec![Version::V1],
        )
        .await;

        Ok(Self {
            name: name.to_string(),
            version: version.to_string(),
            log_prefix,
            subject_group,
            endpoints_by_version,
        })
    }

    /// Adds a new version grouping for endpoints
    pub async fn add_new_service_versions(&self, versions: Vec<Version>) {
        let current_version_map = self.clone().endpoints_by_version;
        let new_version_map = MicroService::add_endpoint_versions(
            current_version_map,
            self.clone().subject_group,
            versions,
        )
        .await;

        self.clone().endpoints_by_version = new_version_map;
    }

    /// Performs a health check for the provided version group
    pub async fn add_version_health_endpoint(&self, version: &str) {
        let handler: EndpointHandler =
            Arc::new(|_msg: &Message| -> Result<Vec<u8>, anyhow::Error> {
                serde_json::to_vec(&"OK").map_err(|_| anyhow::anyhow!("Error"))
            });
        let _ = self
            .add_endpoint("health", version, EndpointType::Sync(handler))
            .await;
    }

    /// Adds a service endpoint for the provided version group
    pub async fn add_endpoint(
        &self,
        e_subject: &str,
        version: &str,
        endpoint_type: EndpointType,
    ) -> Result<(), async_nats::Error> {
        let endpoint_version = Version::from(version.to_string());
        let service_log_prefix = self.log_prefix.clone();
        let current_state = self.clone();

        // Add endpoint to version, if version exists
        if let Some(version_group) = self
            .endpoints_by_version
            .lock()
            .await
            .get_mut(&endpoint_version)
        {
            // Register NATS subject subscription
            let endpoint = MicroService::add_group_endpoint(
                current_state.clone(),
                version_group,
                e_subject,
                endpoint_type,
                endpoint_version,
            )
            .await?;

            // Handle NATS subject subscription (as endpoint)
            self.spawn_endpoint(
                endpoint,
                Version::from(version.to_string()),
                e_subject.to_string(),
            )
            .await;
        } else {
            log::warn!(
                "{}Version does not exist for service.  Unable to add endpoint.",
                service_log_prefix
            );
        };

        Ok(())
    }

    /// Runs handler for endpoint
    pub async fn spawn_endpoint(
        &self,
        mut endpoint: Endpoint,
        endpoint_version: Version,
        e_subject: String,
    ) {
        let service_log_prefix = self.log_prefix.clone();
        let service_name = self.name.clone();
        let version_string: String = endpoint_version.to_string();

        if let Some(version_group) = self
            .endpoints_by_version
            .lock()
            .await
            .get(&endpoint_version)
        {
            let version_group = version_group.clone();

            tokio::spawn(async move {
                while let Some(request) = endpoint.next().await {
                    log::trace!(
                        "{}Service endpoint received message: subj='{}.{}', service={}",
                        service_log_prefix,
                        version_string,
                        e_subject,
                        service_name
                    );

                    if let Some(endpoint_type) =
                        version_group.endpoints.read().await.get(&e_subject)
                    {
                        let result = match endpoint_type {
                            EndpointType::Sync(handler) => handler(&request.message),
                            EndpointType::Async(handler) => handler(&request.message).await,
                        };

                        match result {
                            Ok(response) => {
                                // NOTE: Only return a response if a reply address exists,
                                // otherwise, the underlying NATS system will receive a message it can't
                                // broker and will panic!
                                if let Some(_reply) = &request.message.reply {
                                    request
                                        .respond(Ok(response.into()))
                                        .await
                                        .expect("Failed to send response");
                                } else {
                                    log::warn!(
                                        "{}Ignoring the reply to a message sent without a reply address: subj='{}.{}', message={:?}, service={}",
                                        service_log_prefix,
                                        version_string,
                                        e_subject,
                                        request.message,
                                        service_name,
                                    )
                                }
                            }
                            Err(err) => {
                                log::error!(
                                    "{}Failed to handle a message: subj='{}.{}', message={:?}, service={}, err={:?}",
                                    service_log_prefix,
                                    version_string,
                                    e_subject,
                                    request.message,
                                    service_name,
                                    err
                                );
                                let e = async_nats::service::error::Error {
                                    status: format!("Error: {}", err),
                                    code: 500,
                                };
                                if let Some(_reply) = &request.message.reply {
                                    request
                                        .respond(Err(e))
                                        .await
                                        .expect("Failed to send response");
                                }
                            }
                        }
                    }
                }
            });
        } else {
            log::warn!(
                "{}Version does not exist for service. Unable to spawn endpoint handler. version={}, service={}",
                service_log_prefix,
                version_string,
                service_name,
            );
        };
    }

    /// Fetches stats for endpoints
    // TODO: Update with real calculations
    fn handle_stats(endpoint: String, stats: endpoint::Stats) -> serde_json::Value {
        let stats_json = json!({
            "endpoint": endpoint,
            "requests": stats.requests,
            "errors": stats.errors,
            "average_processing_time": stats.processing_time.as_millis(),
        });
        log::debug!("Stats for {}: {:#?}", endpoint, stats_json.to_string());
        stats_json
    }

    /// Helper fn for adding new version grouping for endpoints
    async fn add_endpoint_versions(
        version_map: Arc<Mutex<HashMap<Version, GroupExt>>>,
        subject_group: Arc<Group>,
        versions: Vec<Version>,
    ) -> Arc<Mutex<HashMap<Version, GroupExt>>> {
        for version in versions.into_iter() {
            let endpoint_info = GroupExt {
                group: Arc::new(RwLock::new(subject_group.group(version.to_string()))),
                endpoints: Arc::new(RwLock::new(HashMap::new())),
            };
            version_map.lock().await.insert(version, endpoint_info);
        }
        version_map
    }

    /// Helper fn for adding new endpoints
    async fn add_group_endpoint(
        self,
        version_group: &mut GroupExt,
        e_subject: &str,
        endpoint_type: EndpointType,
        endpoint_version: Version,
    ) -> Result<Endpoint, async_nats::Error> {
        // Subscribe NATS group to new NATS subject
        let endpoint = version_group
            .group
            .read()
            .await
            .endpoint(e_subject.to_string())
            .await?;

        // Add endpoint handler for NATS subject to endpoint list
        version_group
            .endpoints
            .write()
            .await
            .insert(e_subject.to_string(), endpoint_type);

        // Update state with new endpoint handler fn
        if let Some(state_version_group) = self
            .endpoints_by_version
            .lock()
            .await
            .get_mut(&endpoint_version)
        {
            *state_version_group = version_group.to_owned();
        };

        Ok(endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_nats::ConnectOptions;
    use std::process::{Child, Command};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time;

    const TEST_NATS_SERVER_URL: &str = "nats://127.0.0.1:5222";
    const SERVICE_SEMVER: &str = "0.0.1";
    const SERVICE_NAME: &str = "test_service";
    const SUBJECT: &str = "test.subject";
    const DESCRIPTION: &str = "Test MicroService";

    /// Starts a test NATS server in a separate process
    fn start_test_nats_server() -> Child {
        Command::new("nats-server")
            .args(["-p", "5222", "-js"])
            .spawn()
            .expect("Failed to start NATS server")
    }

    /// Stop the test NATS server
    async fn stop_test_nats_server(mutex_child: Arc<Mutex<Option<Child>>>) {
        let mut maybe_child = mutex_child.lock().await;
        if let Some(child) = maybe_child.as_mut() {
            // Wait for the server process to finish
            let status = child.kill().expect("Failed to kill server");
            println!("NATS server exited with status: {:?}", status);
        } else {
            println!("No running server to shut down.");
        }
    }

    #[tokio::test]
    #[ignore = "todo: resolve server spawn issue"]
    async fn test_microservice_init() {
        // Setup test server in a separate thread
        let test_server: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let test_server_clone = test_server.clone();
        let _ = tokio::spawn(async move {
            let mut test_server = test_server_clone.lock().await;
            *test_server = Some(start_test_nats_server());

            // Wait for the server to start
            time::sleep(Duration::from_secs(2)).await;
        });

        log::info!("Running client test suite");

        //  Test init client
        let client = ConnectOptions::new()
            .name("test_client")
            .connect(TEST_NATS_SERVER_URL)
            .await
            .expect("Failed to connect to NATS");

        let microservice = MicroService::new(
            client.clone(),
            SERVICE_NAME,
            SUBJECT,
            DESCRIPTION,
            SERVICE_SEMVER,
        )
        .await
        .expect("Failed to initialize MicroService");

        assert_eq!(microservice.name, SERVICE_NAME);
        assert_eq!(microservice.version, SERVICE_SEMVER);

        // test_adding new service versions
        microservice
            .add_new_service_versions(vec![Version::V2, Version::V3])
            .await;

        let endpoints_by_version = microservice.endpoints_by_version.lock().await;
        assert!(endpoints_by_version.contains_key(&Version::V2));
        assert!(endpoints_by_version.contains_key(&Version::V3));

        // Test adding an endpoint
        let endpoint_subject = "test_endpoint";
        let endpoint_handler: EndpointHandler = Arc::new(|_msg| Ok(vec![1, 2, 3]));
        let endpoint_type = EndpointType::Sync(endpoint_handler);

        let result = microservice
            .add_endpoint(
                endpoint_subject,
                &Version::V1.to_string(),
                endpoint_type.clone(),
            )
            .await;
        assert!(result.is_ok(), "Failed to add endpoint");

        // Test spawning the endpoint
        microservice
            .add_endpoint(endpoint_subject, &Version::V1.to_string(), endpoint_type)
            .await
            .expect("Failed to add endpoint");

        let endpoint = microservice
            .endpoints_by_version
            .lock()
            .await
            .get(&Version::V1)
            .unwrap()
            .group
            .read()
            .await
            .endpoint(endpoint_subject.to_string())
            .await
            .expect("Failed to get micorservice endpoint");

        microservice
            .spawn_endpoint(endpoint, Version::V1, endpoint_subject.to_string())
            .await;

        // Test adding the version health endpoint (for the requested version)
        let version_groups = microservice.endpoints_by_version.lock().await;
        let version_group = version_groups
            .get(&Version::V1)
            .expect("Version group does not exist");

        let endpoints = version_group.endpoints.read().await;
        assert!(endpoints.contains_key("health"));

        // Close client
        client
            .drain()
            .await
            .expect("Failed to drain, flush and close client");

        // close server
        stop_test_nats_server(test_server).await;
    }
}
