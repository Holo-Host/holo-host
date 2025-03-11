#[macro_export]
macro_rules! generate_service_call {
    ($api:expr, $method_name:ident) => {{
        let api = $api.clone();
        std::sync::Arc::new(move |msg: std::sync::Arc<async_nats::Message>| {
            let api_clone = api.clone();
            Box::pin(async move { api_clone.$method_name(msg).await })
        })
    }};
}
