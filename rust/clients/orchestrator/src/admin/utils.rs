use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;
use nats_utils::{
    jetstream_service::JsStreamService,
    types::{EndpointTraits, ResponseSubjectsGenerator, ServiceConsumerBuilder},
};
use serde::Serialize;

// TODO(decide): either create a service that handles these or refactor error handling altogether
pub const WORKLOAD_ERROR_INBOX_SUBJECT: &str = "WORKLOAD.ERROR.INBOX";

/// TODO(fix): `is_prefix == false` is considered an error case
// TODO(correctness): ensure these errors are forwarded to a human being in some way
pub fn create_callback_subject_to_host(
    is_prefix: bool,
    tag_name: String,
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |tag_map: HashMap<String, String>| -> Vec<String> {
        if !is_prefix {
            log::error!(
                "WORKLOAD Error: Failed to find {tag_name}.
            Unable to send orchestrator response to hosting agent for subject {sub_subject_name}.
            Fwding response to `{WORKLOAD_ERROR_INBOX_SUBJECT}`."
            );
            return vec![WORKLOAD_ERROR_INBOX_SUBJECT.to_string()];
        }

        tag_map
            .into_iter()
            .filter_map(|(k, v)| {
                if k.starts_with(&tag_name)
                // TODO(double-check): this condition is probbaly redudant
                || k == tag_name
                {
                    Some(format!("{v}.{sub_subject_name}"))
                } else {
                    None
                }
            })
            .collect()
    })
}


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
