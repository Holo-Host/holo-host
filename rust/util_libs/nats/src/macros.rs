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
                        log::error!("{e}");
                        let a = anyhow::Error::from(e);
                        log::error!("{a}");
                        match a.downcast::<ServiceError>() {
                            Ok(se) => se.clone(),
                            Err(e) => ServiceError::Internal {
                                message: e.to_string(),
                                context: None,
                            },
                        }
                    })
            })
        })
    }};
}
