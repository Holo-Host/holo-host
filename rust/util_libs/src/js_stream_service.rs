use super::nats_js_client::EndpointType;

use anyhow::{anyhow, Result};
use std::any::Any;
// use async_nats::jetstream::message::Message;
use async_trait::async_trait;
use async_nats::jetstream::consumer::{self, AckPolicy, PullConsumer};
use async_nats::jetstream::stream::{self, Info, Stream};
use async_nats::jetstream::Context;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

type ResponseSubjectsGenerator = Arc<dyn Fn(Option<Vec<String>>) -> Vec<String> + Send + Sync>;

pub trait CreateTag: Send + Sync {
    fn get_tags(&self) -> Option<Vec<String>>;
}

pub trait EndpointTraits:  Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + Debug + CreateTag + 'static {}

#[async_trait]
pub trait ConsumerExtTrait: Send + Sync + Debug + 'static {
    fn get_name(&self) -> &str;
    fn get_consumer(&self) -> PullConsumer;
    fn get_endpoint(&self) -> Box<dyn Any + Send + Sync>;
    fn get_response(&self) -> Option<ResponseSubjectsGenerator>;
}

impl<T> TryFrom<Box<dyn Any + Send + Sync>> for EndpointType<T>
where
    T: EndpointTraits,
{
    type Error = anyhow::Error;

    fn try_from(value: Box<dyn Any + Send + Sync>) -> Result<Self, Self::Error> {
        if let Ok(endpoint) = value.downcast::<EndpointType<T>>() {
            Ok(*endpoint)
        } else {
            Err(anyhow::anyhow!("Failed to downcast to EndpointType"))
        }
    }
}

#[derive(Clone, derive_more::Debug)]
pub struct ConsumerExt<T> 
where
    T: EndpointTraits,
{
    name: String,
    consumer: PullConsumer,
    handler: EndpointType<T>,
    #[debug(skip)]
    response_subject_fn: Option<ResponseSubjectsGenerator>
}

#[async_trait]
impl<T> ConsumerExtTrait for ConsumerExt<T> 
where
    T: EndpointTraits,
{
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_consumer(&self) -> PullConsumer {
        self.consumer.clone()
    }
    fn get_endpoint(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(self.handler.clone())
    }
    fn get_response(&self) -> Option<ResponseSubjectsGenerator> {
        self.response_subject_fn.clone()
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct JsStreamServiceInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub service_subject: &'a str,
}

struct LogInfo {
    prefix: String,
    service_name: String,
    service_subject: String,
    endpoint_name: String,
    endpoint_subject: String,
}

#[derive(Deserialize, Default)]
pub struct JsServiceParamsPartial {
    pub name: String,
    pub description: String,
    pub version: String,
    pub service_subject: String,
}

/// Microservice for Jetstream Streams
// This setup creates only one subject for the stream (eg: "WORKLOAD.>") and sets up
// all consumers of the stream to listen to stream subjects beginning with that subject (eg: "WORKLOAD.start")
#[derive(Clone, Debug)]
pub struct JsStreamService {
    name: String,
    version: String,
    service_subject: String,
    service_log_prefix: String,
    js_context: Arc<RwLock<Context>>,
    stream: Arc<RwLock<Stream<Info>>>,
    local_consumers: Arc<RwLock<HashMap<String, Arc<dyn ConsumerExtTrait>>>>,
}

impl JsStreamService {
    /// Create a new MicroService instance
    // NB: The service consumer creates the stream
    pub async fn new(
        context: Context,
        name: &str,
        description: &str,
        version: &str,
        service_subject: &str,
    ) -> Result<Self, async_nats::Error> 
    where
        Self: 'static
    {
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

    pub fn get_service_info(&self) -> JsStreamServiceInfo {
        JsStreamServiceInfo {
            name: self.name.as_ref(),
            version: self.version.as_ref(),
            service_subject: self.service_subject.as_ref(),
        }
    }

    pub async fn get_consumer_stream_info(&self, consumer_name: &str) -> Result<Option<consumer::Info>> {
        if let Some(consumer_ext) = self
            .to_owned()
            .local_consumers
            .write()
            .await
            .get_mut(&consumer_name.to_string())
        {
            let mut consumer = consumer_ext.get_consumer();
            let info = consumer.info().await?;
            Ok(Some(info.to_owned()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_consumer<T>(&self, consumer_name: &str) -> Result<ConsumerExt<T>>
    where
        T: EndpointTraits,
    {
        let consumer_ext = self
            .local_consumers
            .read()
            .await
            .get(&consumer_name.to_string())
            .ok_or(anyhow!("Error"))?
            .to_owned();

        let endpoint_trait_obj = consumer_ext.get_endpoint();
        let handler: EndpointType<T> = EndpointType::try_from(endpoint_trait_obj)?;

        Ok(ConsumerExt {
            name: consumer_ext.get_name().to_string(),
            consumer:consumer_ext.get_consumer(),
            handler,
            response_subject_fn: consumer_ext.get_response()
        })
    }

    pub async fn add_local_consumer<T>(
        &self,
        consumer_name: &str,
        endpoint_subject: &str,
        endpoint_type: EndpointType<T>,
        response_subject_fn: Option<ResponseSubjectsGenerator>,
    ) -> Result<ConsumerExt<T>, async_nats::Error>  
    where
        T: EndpointTraits,
    {
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
            response_subject_fn,
        };

        self.local_consumers
            .write()
            .await
            .insert(consumer_name.to_string(), Arc::new(consumer_with_handler));

        let endpoint_consumer: ConsumerExt<T> = self.get_consumer(consumer_name).await?;
        self.spawn_consumer_handler::<T>(consumer_name).await?;

        log::debug!(
            "{}Added the {} local consumer",
            self.service_log_prefix,
            endpoint_consumer.name,
        );

        Ok(endpoint_consumer)
    }

    pub async fn spawn_consumer_handler<T>(
        &self,
        consumer_name: &str,
    ) -> Result<(), async_nats::Error> 
    where
        T: EndpointTraits,
    {
        if let Some(consumer_ext) = self
        .to_owned()
        .local_consumers
        .write()
        .await
        .get_mut(&consumer_name.to_string())
        {
            let consumer_details = consumer_ext.to_owned();
            let endpoint_handler: EndpointType<T> = EndpointType::try_from(consumer_details.get_endpoint())?;
            let maybe_response_generator = consumer_ext.get_response();
            let mut consumer = consumer_details.get_consumer();
            let messages = consumer
                .stream()
                .heartbeat(std::time::Duration::from_secs(10))
                .messages()
                .await?;
            
            let log_info = LogInfo {
                prefix: self.service_log_prefix.clone(),
                service_name: self.name.clone(),
                service_subject: self.service_subject.clone(),
                endpoint_name: consumer_details.get_name().to_owned(),
                endpoint_subject: consumer
                .info()
                .await?
                .config
                .filter_subject
                .clone()
            };
            
            let service_context = self.js_context.clone();
            
            tokio::spawn(async move {
                Self::process_messages(
                    log_info,
                    service_context,
                    messages,
                    endpoint_handler,
                    maybe_response_generator,
                )
                .await;
            });
        } else {
            log::warn!(
                "{}Unable to spawn the consumer endpoint handler. Consumer does not exist in the stream service: consumer={}, service={}",
                self.service_log_prefix,
                consumer_name,
                self.name

            );
        };

        Ok(())
    }

    async fn process_messages<T>(
        log_info: LogInfo,
        service_context: Arc<RwLock<Context>>,
        mut messages: consumer::pull::Stream,
        endpoint_handler: EndpointType<T>,
        maybe_response_generator: Option<ResponseSubjectsGenerator>,
    ) where
        T: EndpointTraits,
    {
        while let Some(Ok(js_msg)) = messages.next().await {
            log::trace!(
                "{}Consumer received message: subj='{}.{}', endpoint={}, service={}",
                log_info.prefix,
                log_info.service_subject,
                log_info.endpoint_subject,
                log_info.endpoint_name,
                log_info.service_name
            );

            let result = match endpoint_handler {
                EndpointType::Sync(ref handler) => handler(&js_msg.message),
                EndpointType::Async(ref handler) => {
                    handler(Arc::new(js_msg.clone().message)).await
                }
            };

            let (response_bytes, maybe_subject_tags) = match result {
                Ok(r) => {
                    let bytes: bytes::Bytes = match serde_json::to_vec(&r) {
                        Ok(r) => r.into(),
                        Err(e) => e.to_string().into()
                    };
                    let maybe_subject_tags = r.get_tags();
                    (bytes, maybe_subject_tags)
                },
                Err(err) => (err.to_string().into(), None),
            };

            // Returns a response if a reply address exists.
            // (Note: This means the js subject was called with a `req` instead of a `pub`.)
            if let Some(reply) = &js_msg.reply {
                if let Err(err) = service_context
                    .read()
                    .await
                    .publish(
                        format!("{}.{}.{}", reply, log_info.service_subject, log_info.endpoint_subject),
                        response_bytes.clone(),
                    )
                    .await
                {
                    log::error!(
                        "{}Failed to send reply upon successful message consumption: subj='{}.{}.{}', endpoint={}, service={}, err={:?}",
                        log_info.prefix,
                        reply,
                        log_info.service_subject,
                        log_info.endpoint_subject,
                        log_info.endpoint_name,
                        log_info.service_name,
                        err
                    );

                    // todo: discuss how we want to handle error
                };
            }

            // Publish a response message to response subjects when an endpoint response subject generator exists for endpoint
            if let Some(response_subject_fn) = maybe_response_generator.as_ref() {
                let response_subjects = response_subject_fn(maybe_subject_tags);
                for response_subject in response_subjects.iter() {
                    if let Err(err) = service_context
                        .read()
                        .await
                        .publish(
                            format!("{}.{}", log_info.service_subject, response_subject),
                            response_bytes.clone(),
                        )
                        .await
                    {
                        log::error!(
                            "{}Failed to publish new message upon successful message consumption: subj='{}.{}', endpoint={}, service={}, err={:?}",
                            log_info.prefix,
                            log_info.service_subject,
                            log_info.endpoint_subject,
                            log_info.endpoint_name,
                            log_info.service_name,
                            err
                        );
                    };
                }
                // todo: discuss how we want to handle error
            }

            // Send back message acknowledgment
            if let Err(err) = js_msg.ack().await {
                log::error!(
                    "{}Failed to send ACK new message upon successful message consumption: subj='{}.{}', endpoint={}, service={}, err={:?}",
                    log_info.prefix,
                    log_info.service_subject,
                    log_info.endpoint_subject,
                    log_info.endpoint_name,
                    log_info.service_name,
                    err
                );

                // todo: discuss how we want to handle error
            }
        }
    }

}

#[cfg(feature = "tests_integration_nats")]
#[cfg(test)]
mod tests {
    use super::*;
    use async_nats::{jetstream, ConnectOptions};
    use std::sync::Arc;

    const NATS_SERVER_URL: &str = "nats://localhost:4222";
    const SERVICE_SEMVER: &str = "0.0.1";

    pub async fn setup_jetstream() -> Context {
        let client = ConnectOptions::new()
            .name("test_client")
            .connect(NATS_SERVER_URL)
            .await
            .expect("Failed to connect to NATS");

        jetstream::new(client)
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
