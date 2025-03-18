use anyhow::Result;
use nats_utils::{
    jetstream_service::JsStreamService,
    types::{EndpointTraits, ResponseSubjectsGenerator, ServiceConsumerBuilder},
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn add_workload_consumer<S, R>(
    service_builder: ServiceConsumerBuilder<S, R>,
    workload_service: &JsStreamService,
) -> Result<()>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    workload_service
        .add_consumer(service_builder.into())
        .await
        .map_err(|e| anyhow::Error::msg(e.to_string()))?;

    Ok(())
}

// TODO: either create a service that handles these or refactor error handling altogether
pub const WORKLOAD_ERROR_INBOX_SUBJECT: &str = "WORKLOAD.ERROR.INBOX";

/// `is_prefix == false` is considered an error case
// TODO(correctness): ensure these errors are forwarded to a human being in some way
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
        log::error!(
            "WORKLOAD Error: Failed to find {tag_name}.
            Unable to send orchestrator response to hosting agent for subject {sub_subject_name}.
            Fwding response to `{WORKLOAD_ERROR_INBOX_SUBJECT}`."
        );
        vec![WORKLOAD_ERROR_INBOX_SUBJECT.to_string()]
    })
}
