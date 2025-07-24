#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use db_utils::mongodb::api::MongoDbAPI;
    use db_utils::schemas::DATABASE_NAME;
    use mock_utils::host::create_test_host;
    use mock_utils::mongodb_runner::MongodRunner;
    use mock_utils::nats_message::NatsMessage;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_handle_host_update() -> Result<()> {
        // TODO: Implement Host's HPOS Update Service API test
        Ok(())
    }
}
