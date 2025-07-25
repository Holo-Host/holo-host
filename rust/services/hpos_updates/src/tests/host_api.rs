#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use mock_utils::nats_message::NatsMessage;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_handle_host_update_good_state() -> Result<()> {
        let api = HostUpdatesApi {};

        let starting_request_info = HostUpdateInfo {
            request_info: HostUpdateRequest {
                channel: "nixos-unstable".to_string(),
                device_id: "test_device_id".to_string(),
            },
            state: HostUpdateState::Pending,
            context: None,
        };

        let msg_payload = serde_json::to_vec(&starting_request_info).unwrap();
        let msg = Arc::new(NatsMessage::new("HPOS.host.update", msg_payload).into_message());

        let result = api.handle_host_update_command(msg).await?;

        assert!(
            result.info.state == HostUpdateState::Completed
                || result.info.state == HostUpdateState::Failed
        );
        assert_eq!(
            result.info.request_info.device_id,
            starting_request_info.request_info.device_id
        );
        assert_eq!(
            result.info.request_info.channel,
            starting_request_info.request_info.channel
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_host_update_response_bad_state() -> Result<()> {
        let api = HostUpdatesApi {};
        let completed_info = HostUpdateInfo {
            request_info: HostUpdateRequest {
                channel: "nixos-unstable".to_string(),
                device_id: "test_device_id".to_string(),
            },
            state: HostUpdateState::Completed,
            context: Some("Update performed successfully".to_string()),
        };
        let msg_payload = serde_json::to_vec(&completed_info).unwrap();
        let msg = Arc::new(NatsMessage::new("HPOS.host.status", msg_payload).into_message());
        let result = api.handle_host_update_command(msg).await?;
        assert_eq!(result.info.state, HostUpdateState::Completed);
        assert_eq!(
            result.info.context,
            Some("Update performed successfully".to_string())
        );
        Ok(())
    }
}
