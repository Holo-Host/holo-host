use crate::types::sanitization::sanity_check_nats_name;
use crate::types::ServiceConsumerBuilder;

use super::types::{
    ConsumerBuilder, ConsumerExt, ConsumerExtTrait, EndpointTraits, EndpointType,
    JsStreamServiceInfo, LogInfo, ResponseSubjectsGenerator,
};
use anyhow::{Context, Result};
use async_nats::jetstream::consumer::{self, AckPolicy};
use async_nats::jetstream::stream::{self, DeleteStatus, Info, Stream};
use async_nats::jetstream::Context as JsContext;
use futures::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Microservice for Jetstream Streams
// This setup creates only one subject for the stream (eg: "WORKLOAD.>") and sets up
// all consumers of the stream to listen to stream subjects beginning with that subject (eg: "WORKLOAD.start")
#[derive(Clone, Debug)]
pub struct JsStreamService {
    name: String,
    version: String,
    service_subject: String,
    service_log_prefix: String,
    js_context: Arc<RwLock<JsContext>>,
    stream: Arc<RwLock<Stream<Info>>>,
    local_consumers: LocalConsumers,
}

pub type LocalConsumers = Arc<RwLock<HashMap<String, (Arc<dyn ConsumerExtTrait>, JoinHandle<()>)>>>;

impl JsStreamService {
    pub const HEADER_NAME_REPLY_OVERRIDE: &str = "reply_override";

    /// Create a new MicroService instance
    // NB: The service consumer creates the stream
    pub async fn new(
        context: JsContext,
        name: &str,
        description: &str,
        version: &str,
        service_subject: &str,
    ) -> Result<Self, async_nats::Error>
    where
        Self: 'static,
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

    pub async fn get_consumer_stream_info(
        &self,
        consumer_name: &str,
    ) -> Result<Option<consumer::Info>> {
        if let Some((consumer_ext, _)) = self
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

    /// Add a consumer to the NATS server, and store the information and spawned handler.
    pub async fn add_consumer<T>(
        &self,
        builder_params: ConsumerBuilder<T>,
    ) -> Result<Arc<ConsumerExt<T>>, async_nats::Error>
    where
        T: EndpointTraits,
    {
        // Add the Service Subject prefix
        let consumer_subject = format!("{}.{}", self.service_subject, builder_params.subject);

        log::debug!("adding consumer with subject {consumer_subject}");

        let consumer_name = builder_params.name.to_string();

        sanity_check_nats_name(&consumer_name)?;

        // Register JS Subject Consumer
        let consumer_config = consumer::pull::Config {
            durable_name: Some(builder_params.name.to_string()),
            ack_policy: AckPolicy::Explicit,
            filter_subject: consumer_subject.clone(),
            ..Default::default()
        };

        let consumer = self
            .stream
            .write()
            .await
            .get_or_create_consumer(&builder_params.name, consumer_config.clone())
            .await
            .context(format!(
                "get_or_create_consumer {} with config {consumer_config:?}",
                &builder_params.name,
            ))?;

        // TODO(post-bug) adding the Arc around the consumer with handler maybe solved the consumer timeout bug
        let consumer_with_handler = Arc::new(ConsumerExt {
            consumer,
            handler: builder_params.handler,
            response_subject_fn: builder_params.response_subject_fn,
        });

        let handle = self
            .spawn_consumer_handler::<T>(
                Arc::clone(&consumer_with_handler) as Arc<dyn ConsumerExtTrait>
            )
            .await?;

        if let Some(_previous_consumer) = self.local_consumers.write().await.insert(
            builder_params.name.to_string(),
            (
                Arc::clone(&consumer_with_handler) as Arc<dyn ConsumerExtTrait>,
                handle,
            ),
        ) {
            log::debug!(
                "found previous local consumer with name {}",
                &builder_params.name
            );

            // TODO(correctness): clean it up if this was the last usage of the consumer?
        };

        log::debug!(
            "{}Added the {} local consumer with subject {consumer_subject}",
            self.service_log_prefix,
            builder_params.name,
        );

        Ok(consumer_with_handler)
    }

    /// Deletes the given consumer and aborts its handle.
    /// Indicates the existence of the given consumer name via the return value.
    pub async fn delete_consumer(&self, name: &str) -> bool {
        match self.local_consumers.write().await.entry(name.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                log::debug!("removing consumer {name} and aborting handle.");

                let (stored_name, (_, handle)) = entry.remove_entry();

                // delete the consumer on the NATS server
                match self
                    .stream
                    .write()
                    .await
                    .delete_consumer(&stored_name)
                    .await
                {
                    Ok(DeleteStatus { success }) => {
                        log::debug!("delete status for consumer {stored_name}: {success}")
                    }
                    Err(e) => {
                        log::warn!("error deleting consumer {stored_name}: {e}")
                    }
                };

                handle.abort();

                true
            }
            std::collections::hash_map::Entry::Vacant(_) => false,
        }
    }

    pub async fn spawn_consumer_handler<T>(
        &self,
        consumer_details: Arc<dyn ConsumerExtTrait>,
    ) -> Result<tokio::task::JoinHandle<()>, async_nats::Error>
    where
        T: EndpointTraits,
    {
        let endpoint_handler: EndpointType<T> =
            EndpointType::try_from(consumer_details.get_endpoint())?;
        let maybe_response_subject_generator = consumer_details.get_response_subject_fn();
        let mut consumer = consumer_details.get_consumer();
        let messages = consumer
            .stream()
            .heartbeat(std::time::Duration::from_secs(10))
            .max_messages_per_batch(100)
            .expires(std::time::Duration::from_secs(30))
            .messages()
            .await
            .context("getting consumer messages")?;

        let consumer_info = consumer.info().await?;

        let log_info = LogInfo {
            prefix: self.service_log_prefix.clone(),
            service_name: self.name.clone(),
            service_subject: self.service_subject.clone(),
            endpoint_name: consumer_info
                .config
                .durable_name
                .clone()
                .unwrap_or("Consumer Name Not Found".to_string())
                .clone(),
            endpoint_subject: consumer_info.config.filter_subject.clone(),
        };

        let handle = tokio::spawn({
            let js_context = Arc::clone(&self.js_context);
            async move {
                let _ = Self::process_messages(
                    log_info,
                    js_context,
                    messages,
                    endpoint_handler,
                    maybe_response_subject_generator,
                )
                .await;
            }
        });

        Ok(handle)
    }

    async fn process_messages<T>(
        log_info: LogInfo,
        service_context: Arc<RwLock<JsContext>>,
        mut messages: consumer::pull::Stream,
        endpoint_handler: EndpointType<T>,
        maybe_response_subject_generator: Option<ResponseSubjectsGenerator>,
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

            // TODO(learning; author: stefan): on which level do sync vs async play out?
            let result = match endpoint_handler {
                EndpointType::Sync(ref handler) => handler(&js_msg.message),
                EndpointType::Async(ref handler) => handler(Arc::new(js_msg.clone().message)).await,
            };

            let (response_bytes, maybe_subject_tags) = match result {
                Ok(r) => (r.get_response(), r.get_subject_tags()),
                Err(err) => (err.to_string().into(), HashMap::new()),
            };

            let maybe_reply = match (&js_msg.headers, &js_msg.reply) {
                (None, None) => None,
                (None, Some(reply)) => Some(reply.to_string()),
                (Some(headers), None) => headers.get("reply_override").map(ToString::to_string),

                // prefer the reply_override header
                (Some(headers), Some(reply)) => headers
                    .get(Self::HEADER_NAME_REPLY_OVERRIDE)
                    .map(ToString::to_string)
                    .or(Some(reply.to_string())),
            };

            // Returns a response if a reply subject exists.
            // (Note: This means the js subject was called with a `req` instead of a `pub`.)
            if let Some(reply) = maybe_reply {
                log::debug!("publishing reply to {reply}");
                if let Err(err) = service_context
                    .read()
                    .await
                    .publish(reply.clone(), response_bytes.clone())
                    .await
                {
                    log::error!(
                        "{}Failed to send reply upon successful message consumption: subj='{}', endpoint={}, service={}, err={:?}",
                        log_info.prefix,
                        reply,
                        log_info.endpoint_name,
                        log_info.service_name,
                        err
                    );

                    // todo: discuss how we want to handle error
                };
            }

            // Publish a response message to response subjects when an endpoint response subject generator exists for endpoint
            if let Some(response_subject_fn) = maybe_response_subject_generator.as_ref() {
                let response_subjects = response_subject_fn(maybe_subject_tags);
                for response_subject in response_subjects.iter() {
                    // TODO(simplify): remove the service_subject?
                    let subject = format!("{}.{}", log_info.service_subject, response_subject);

                    log::debug!("publishing a response on {subject}");

                    if let Err(err) = service_context
                        .read()
                        .await
                        .publish(subject, response_bytes.clone())
                        .await
                    {
                        log::error!(
                            "{}Failed to publish new message upon successful message consumption: subj='{}.{}', endpoint={}, service={}, err={:?}",
                            log_info.prefix,
                            log_info.service_subject,
                            response_subject,
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

    pub async fn add_workload_consumer<S, R>(
        &self,
        service_builder: ServiceConsumerBuilder<S, R>,
    ) -> Result<()>
    where
        S: Serialize + Clone + AsRef<str>,
        R: EndpointTraits,
    {
        self.add_consumer(service_builder.into())
            .await
            .map_err(|e| anyhow::Error::msg(e.to_string()))?;

        Ok(())
    }
}
