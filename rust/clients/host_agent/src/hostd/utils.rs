use nats_utils::types::ResponseSubjectsGenerator;
use std::{collections::HashMap, sync::Arc};

pub fn create_callback_subject_to_orchestrator(
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        vec![format!("orchestrator.{}", sub_subject_name)]
    })
}
