use nats_utils::types::{
    AsyncEndpointHandler, ConsumerBuilder, EndpointTraits, EndpointType, ResponseSubjectsGenerator,
};
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

pub fn create_callback_subject_to_host(
    is_prefix: bool,
    tag_name: String,
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |tag_map: HashMap<String, String>| -> Vec<String> {
        if is_prefix {
            let matching_tags = tag_map.into_iter().fold(vec![], |mut acc, (k, v)| {
                if k.starts_with(&tag_name) {
                    acc.push(v)
                }
                acc
            });
            return matching_tags;
        } else if let Some(tag) = tag_map.get(&tag_name) {
            return vec![format!("{}.{}", tag, sub_subject_name)];
        }
        log::error!("WORKLOAD Error: Failed to find {}. Unable to send orchestrator response to hosting agent for subject {}. Fwding response to `WORKLOAD.ERROR.INBOX`.", tag_name, sub_subject_name);
        vec!["WORKLOAD.ERROR.INBOX".to_string()]
    })
}
