use std::{collections::HashMap, sync::Arc};
use util_libs::nats::types::ResponseSubjectsGenerator;

pub fn create_callback_subject_to_orchestrator(
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        vec![format!("orchestrator.{}", sub_subject_name)]
    })
}
