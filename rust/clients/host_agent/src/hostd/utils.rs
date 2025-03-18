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

pub fn create_callback_subject(sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        // TODO(correctness): update this
        vec![format!("orchestrator.{}", sub_subject_name)]
    })
}
