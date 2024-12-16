use crate::nats_client::EndpointType;
use anyhow::{anyhow, Result};
use async_nats::jetstream::consumer::{self, AckPolicy, PullConsumer};
use async_nats::jetstream::stream::{self, Info, Stream};
use async_nats::jetstream::{self, Context};
use async_nats::Client;
use futures::StreamExt;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

impl std::fmt::Debug for ConsumerExt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let handler_placeholder = match &self.handler {
            EndpointType::Async(_) => "EndpointType::Async(<function>)",
            EndpointType::Sync(_) => "EndpointType::Sync(<function>)",
        };

        f.debug_struct("ConsumerExt")
            .field("name", &self.name)
            .field("consumer", &self.consumer)
            .field("handler", &format_args!("{}", handler_placeholder))
            .field("response_subject", &self.response_subject)
            .finish()
    }
}

// This setup expects that each consumer only listens to *one* subject on the stream
#[derive(Clone)]
pub struct ConsumerExt {
    name: String,
    consumer: PullConsumer,
    handler: EndpointType,
    response_subject: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct JsStreamServiceInfo<'a> {
    name: &'a str,
    version: &'a str,
    service_subject: &'a str,
}

/// Microservice for Jetstream Streams
#[derive(Clone, Debug)]
pub struct JsStreamService {
    name: String,
    version: String,
    service_subject: String,
    service_log_prefix: String,
    js_context: Arc<RwLock<Context>>,
    stream: Arc<RwLock<Stream<Info>>>,
    local_consumers: Arc<RwLock<HashMap<String, ConsumerExt>>>,
}

impl JsStreamService {
    // Spin up a jetstream associated with the provided Nats Client
    // NB: The context creation is separated out to allow already established js contexts to be passed into `new` instead of being re/created there.
    pub fn get_context(client: Client) -> Context {
        jetstream::new(client)
    }

    pub fn get_info(&self) -> JsStreamServiceInfo {
        JsStreamServiceInfo {
            name: self.name.as_ref(),
            version: self.version.as_ref(),
            service_subject: self.service_subject.as_ref(),
        }
    }

    /// Create a new MicroService instance
    pub async fn new(
        context: Context,
        name: &str,
        description: &str,
        version: &str,
        service_subject: &str,
    ) -> Result<Self, async_nats::Error> {
        let stream = context
            .get_or_create_stream(&stream::Config {
                name: name.to_string(),
                description: Some(description.to_string()),
                subjects: vec![format!("{}.>", service_subject)],
                allow_direct: true,
                ..Default::default()
            })
            .await?;

        let service_log_prefix = format!("JS-LOG::{}::", name);

        Ok(JsStreamService {
            name: name.to_string(),
            version: version.to_string(),
            service_subject: service_subject.to_string(),
            service_log_prefix,
            js_context: Arc::new(RwLock::new(context)),
            stream: Arc::new(RwLock::new(stream)),
            local_consumers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn with_existing_stream(
        context: Context,
        version: &str,
        stream_name: &str,
    ) -> Result<Self, async_nats::Error> {
        let stream = context.get_stream(stream_name).await?;
        let stream_config = stream.get_info().await?.config;
        let service_log_prefix = format!("LOG::{}:: : ", stream_config.name);
        let service_subject = stream_config.subjects[0].split(">").collect::<Vec<&str>>()[0];

        Ok(JsStreamService {
            name: stream_config.name.to_string(),
            version: version.to_string(),
            service_subject: service_subject.to_string(),
            service_log_prefix,
            js_context: Arc::new(RwLock::new(context)),
            stream: Arc::new(RwLock::new(stream)),
            local_consumers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn add_local_consumer(
        &self,
        consumer_name: &str,
        endpoint_subject: &str,
        endpoint_type: EndpointType,
        response_subject: Option<String>,
    ) -> Result<ConsumerExt, async_nats::Error> {
        let full_subject = format!("{}.{}", self.service_subject, endpoint_subject);

        // Register JS Subject Consumer
        let consumer_config = consumer::pull::Config {
            durable_name: Some(consumer_name.to_string()),
            ack_policy: AckPolicy::Explicit,
            filter_subject: full_subject,
            ..Default::default()
        };

        let consumer = self
            .stream
            .write()
            .await
            .get_or_create_consumer(consumer_name, consumer_config)
            .await?;

        let consumer_with_handler = ConsumerExt {
            name: consumer_name.to_string(),
            consumer,
            handler: endpoint_type,
            response_subject,
        };

        self.local_consumers
            .write()
            .await
            .insert(consumer_name.to_string(), consumer_with_handler);

        let endpoint_consumer: ConsumerExt = self.get_consumer(consumer_name).await?;
        self.spawn_consumer_handler(consumer_name).await?;

        log::debug!(
            "{}Added the {} local consumer",
            self.service_log_prefix,
            endpoint_consumer.name,
        );

        Ok(endpoint_consumer)
    }

    pub async fn spawn_consumer_handler(
        &self,
        consumer_name: &str,
    ) -> Result<(), async_nats::Error> {
        let service_log_prefix = self.service_log_prefix.clone();
        let service_name = self.name.clone();
        let service_subject = self.service_subject.clone();
        let service_context = self.js_context.clone();

        if let Some(consumer_ext) = self
            .to_owned()
            .local_consumers
            .write()
            .await
            .get_mut(&consumer_name.to_string())
        {
            let mut consumer_details = consumer_ext.to_owned();
            let endpoint_response_subject = consumer_details.response_subject.clone();
            let endpoint_name = consumer_details.name.clone();
            let endpoint_subject = consumer_details
                .consumer
                .info()
                .await?
                .config
                .filter_subject
                .clone();

            let mut messages = consumer_details
                .consumer
                .stream()
                .heartbeat(std::time::Duration::from_secs(10))
                .messages()
                .await?;

            tokio::spawn(async move {
                while let Some(Ok(js_msg)) = messages.next().await {
                    log::trace!(
                        "{}Consumer received message: subj='{}.{}', endpoint={}, service={}",
                        service_log_prefix,
                        service_subject,
                        endpoint_subject,
                        endpoint_name,
                        service_name
                    );

                    let result = match consumer_details.handler.to_owned() {
                        EndpointType::Sync(handler) => handler(&js_msg.message),
                        EndpointType::Async(handler) => handler(&js_msg.message).await,
                    };

                    let response_bytes: bytes::Bytes = match result {
                        Ok(response) => response.into(),
                        Err(err) => err.to_string().into(),
                    };

                    // Returns a response if a reply address exists.
                    // (Note: This means the js subject was called with a `req` instead of a `pub`.)
                    if let Some(reply) = &js_msg.reply {
                        if let Err(err) = service_context
                            .read()
                            .await
                            .publish(
                                format!("{}.{}.{}", reply, service_subject, endpoint_subject),
                                response_bytes.clone(),
                            )
                            .await
                        {
                            log::error!(
                                "{}Failed to send reply upon successful message consumption: subj='{}.{}.{}', endpoint={}, service={}, err={:?}",
                                service_log_prefix,
                                reply,
                                service_subject,
                                endpoint_subject,
                                endpoint_name,
                                service_name,
                                err
                            );

                            // todo: discuss how we want to handle error
                        };
                    }

                    // Publish a response message if an endpoint response subject exists for handler
                    if let Some(response_subject) = endpoint_response_subject.as_ref() {
                        if let Err(err) = service_context
                            .read()
                            .await
                            .publish(
                                format!("{}.{}", service_subject, response_subject),
                                response_bytes,
                            )
                            .await
                        {
                            log::error!(
                                "{}Failed to publish new message upon successful message consumption: subj='{}.{}', endpoint={}, service={}, err={:?}",
                                service_log_prefix,
                                service_subject,
                                endpoint_subject,
                                endpoint_name,
                                service_name,
                                err
                            );
                        };

                        // todo: discuss how we want to handle error
                    }

                    // Send back message acknowledgment
                    if let Err(err) = js_msg.ack().await {
                        log::error!(
                            "{}Failed to send ACK new message upon successful message consumption: subj='{}.{}', endpoint={}, service={}, err={:?}",
                            service_log_prefix,
                            service_subject,
                            endpoint_subject,
                            endpoint_name,
                            service_name,
                            err
                        );

                        // todo: discuss how we want to handle error
                    }
                }
            });
        } else {
            log::warn!(
                "{}Unable to spawn the consumer endpoint handler. Consumer does not exist in the stream service: consumer={}, service={}",
                service_log_prefix,
                consumer_name,
                service_name

            );
        };

        Ok(())
    }

    async fn get_consumer(&self, consumer_name: &str) -> Result<ConsumerExt> {
        Ok(self
            .local_consumers
            .read()
            .await
            .get(&consumer_name.to_string())
            .ok_or(anyhow!("Error"))?
            .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_nats::ConnectOptions;
    use std::sync::Arc;

    const NATS_SERVER_URL: &str = "nats://localhost:4222";
    const SERVICE_SEMVER: &str = "0.0.1";

    pub async fn setup_jetstream() -> Context {
        let client = ConnectOptions::new()
            .name("test_client")
            .connect(NATS_SERVER_URL)
            .await
            .expect("Failed to connect to NATS");

        JsStreamService::get_context(client)
    }

    pub async fn get_default_js_service(context: Context) -> JsStreamService {
        JsStreamService::new(
            context,
            "test_service",
            "Test Service",
            SERVICE_SEMVER,
            "test.subject",
        )
        .await
        .expect("Failed to create JsStreamService")
    }

    #[tokio::test]
    async fn test_js_service_init() {
        let context = setup_jetstream().await;
        let service_name = "test_service";
        let description = "Test Service Description";
        let version = SERVICE_SEMVER;
        let subject = "test.subject";

        let service = JsStreamService::new(context, service_name, description, version, subject)
            .await
            .expect("Failed to create JsStreamService");

        assert_eq!(service.name, service_name);
        assert_eq!(service.version, version);
        assert_eq!(service.service_subject, subject);
    }

    #[tokio::test]
    async fn test_js_service_with_existing_stream() {
        let context = setup_jetstream().await;
        let stream_name = "existing_stream";
        let version = SERVICE_SEMVER;

        // Create a stream beforehand
        context
            .get_or_create_stream(&stream::Config {
                name: stream_name.to_string(),
                description: Some("Existing stream description".to_string()),
                subjects: vec![format!("{}.>", stream_name)],
                ..Default::default()
            })
            .await
            .expect("Failed to create stream");

        let service = JsStreamService::with_existing_stream(context, version, stream_name)
            .await
            .expect("Failed to create JsStreamService with existing stream");

        assert_eq!(service.name, stream_name);
        assert_eq!(service.version, version);
    }

    #[tokio::test]
    async fn test_js_service_add_local_consumer() {
        let context = setup_jetstream().await;
        let service = get_default_js_service(context).await;

        let consumer_name = "test_consumer";
        let endpoint_subject = "endpoint";
        let endpoint_type = EndpointType::Sync(Arc::new(|_msg| Ok(vec![1, 2, 3])));
        let response_subject = Some("response.subject".to_string());

        let consumer = service
            .add_local_consumer(
                consumer_name,
                endpoint_subject,
                endpoint_type,
                response_subject,
            )
            .await
            .expect("Failed to add local consumer");

        assert_eq!(consumer.name, consumer_name);
        assert!(consumer.response_subject.is_some());
        assert_eq!(consumer.response_subject.unwrap(), "response.subject");
    }

    #[tokio::test]
    async fn test_js_service_spawn_consumer_handler() {
        let context = setup_jetstream().await;
        let service = get_default_js_service(context).await;

        let consumer_name = "test_consumer";
        let endpoint_subject = "endpoint";
        let endpoint_type = EndpointType::Sync(Arc::new(|_msg| Ok(vec![1, 2, 3])));
        let response_subject = None;

        service
            .add_local_consumer(
                consumer_name,
                endpoint_subject,
                endpoint_type,
                response_subject,
            )
            .await
            .expect("Failed to add local consumer");

        let result = service.spawn_consumer_handler(consumer_name).await;
        assert!(result.is_ok(), "Failed to spawn consumer handler");
    }
}
