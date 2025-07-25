use nats_utils::types::ResponseSubjectsGenerator;
use std::collections::HashMap;
use std::sync::Arc;

// Tag to identify the orchestrator prefix
pub const ORCHESTRATOR_SUBJECT_PREFIX: &str = "orchestrator";

pub fn create_callback_subject(sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        // TODO(refactor): into event subject
        vec![format!("{ORCHESTRATOR_SUBJECT_PREFIX}.{sub_subject_name}",)]
    })
}
