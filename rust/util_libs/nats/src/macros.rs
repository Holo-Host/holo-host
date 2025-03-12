#[macro_export]
macro_rules! generate_service_call {
    ($api:expr, $method_name:ident) => {{
        let api = $api.clone();
        std::sync::Arc::new(move |msg: std::sync::Arc<async_nats::Message>| {
            let api_clone = api.clone();
            Box::pin(async move {
                use nats_utils::types::ServiceError;
                api_clone
                    .$method_name(msg)
                    .await
                    .map_err(|e| -> ServiceError {
                        let a = anyhow::Error::from(e);
                        match a.downcast_ref::<ServiceError>() {
                            Some(se) => se.clone(),
                            None => ServiceError::Internal {
                                message: a.to_string(),
                                context: None,
                            },
                        }
                    })
            })
        })
    }};
}
