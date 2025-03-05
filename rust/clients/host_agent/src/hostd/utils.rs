use nats_utils::types::ResponseSubjectsGenerator;
use nats_utils::types::{AsyncEndpointHandler, ConsumerBuilder, EndpointTraits, EndpointType};
use serde::Serialize;
use std::collections::HashMap;
use std::convert::AsRef;
use std::sync::Arc;

#[derive(Clone)]
pub struct OrchestratorConsumerBuilder<S, R>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    pub name: String,
    pub subject: S,
    pub async_handler: AsyncEndpointHandler<R>,
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
}

pub fn create_consumer<S, R>(w: OrchestratorConsumerBuilder<S, R>) -> ConsumerBuilder<R>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    ConsumerBuilder {
        name: w.name.to_string(),
        subject: w.subject.as_ref().to_string(),
        handler: EndpointType::Async(w.async_handler),
        response_subject_fn: w.response_subject_fn,
    }
}

pub fn create_callback_subject_to_orchestrator(
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        vec![format!("orchestrator.{}", sub_subject_name)]
    })
}
