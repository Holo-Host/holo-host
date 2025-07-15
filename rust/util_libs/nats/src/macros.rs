// TODO(refactor): this is very unclear in case of an error, e.g. the $method_name doesn't exist on the API

#[derive(Clone, Debug)]
pub struct ApiOptions {
    pub device_id: String,
}

#[macro_export]
macro_rules! generate_service_call {
    // Route for API Method with standard args (eg: nats msg arg only)
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

    // Route for API Method with standard args plus additional ApiOptions argument
    ($api:expr, $method_name:ident, $api_options:expr) => {{
        let api = $api.clone();
        let api_options = $api_options.clone();
        std::sync::Arc::new(move |msg: std::sync::Arc<async_nats::Message>| {
            let api_clone = api.clone();
            let api_options_clone = api_options.clone();
            Box::pin(async move {
                use nats_utils::types::ServiceError;
                api_clone
                    .$method_name(msg, api_options_clone)
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
