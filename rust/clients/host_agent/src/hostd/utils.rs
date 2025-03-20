use anyhow::Result;
use nats_utils::{
    jetstream_service::JsStreamService,
    types::{EndpointTraits, ResponseSubjectsGenerator, ServiceConsumerBuilder},
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use workload::WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX;

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

pub fn create_callback_subject_to_orchestrator(
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        // NB: this must match the expected subject for the `OrchestratorWorkloadApi.handle_status_update` consumer
        vec![format!(
            "{WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX}.{sub_subject_name}",
        )]
    })
}
