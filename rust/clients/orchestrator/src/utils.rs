use anyhow::{anyhow, Context, Result};
use nkeys::KeyPair;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc};
use util_libs::nats::types::{
    AsyncEndpointHandler, ConsumerBuilder, EndpointType, ResponseSubjectsGenerator,
};
use workload::types::{WorkloadApiResult, WorkloadServiceSubjects};

#[derive(Clone)]
pub struct WorkloadConsumerBuilder {
    pub name: String,
    pub subject: WorkloadServiceSubjects,
    pub async_handler: AsyncEndpointHandler<WorkloadApiResult>,
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
}

pub fn create_consumer(w: WorkloadConsumerBuilder) -> ConsumerBuilder<WorkloadApiResult> {
    ConsumerBuilder {
        name: w.name.to_string(),
        subject: w.subject.as_ref().to_string(),
        handler: EndpointType::Async(w.async_handler),
        response_subject_fn: w.response_subject_fn,
    }
}

#[macro_export]
macro_rules! generate_call_method {
    ($api:expr, $method_name:ident) => {{
        let api = $api.clone();
        Arc::new(
            move |msg: Arc<Message>| -> util_libs::nats::types::JsServiceResponse<
                workload::types::WorkloadApiResult,
            > {
                let api_clone = api.clone();
                Box::pin(async move { api_clone.$method_name(msg).await })
            },
        )
    }};
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

pub fn try_read_keypair_from_file(key_file_path: PathBuf) -> Result<Option<KeyPair>> {
    match try_read_from_file(key_file_path)? {
        Some(kps) => Ok(Some(KeyPair::from_seed(&kps)?)),
        None => Ok(None),
    }
}

fn try_read_from_file(file_path: PathBuf) -> Result<Option<String>> {
    match file_path.try_exists() {
        Ok(link_is_ok) => {
            if !link_is_ok {
                return Err(anyhow!(
                    "Failed to read path {:?}. Found broken sym link.",
                    file_path
                ));
            }

            let mut file_content = File::open(&file_path)
                .context(format!("Failed to open config file {:#?}", file_path))?;

            let mut s = String::new();
            file_content.read_to_string(&mut s)?;
            Ok(Some(s.trim().to_string()))
        }
        Err(_) => {
            log::debug!("No user file found at {:?}.", file_path);
            Ok(None)
        }
    }
}
