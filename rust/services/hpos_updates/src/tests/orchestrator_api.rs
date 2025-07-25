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
    // use db_utils::schemas::host::Host;

    #[tokio::test]
    async fn test_handle_host_update() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");
        let api = OrchestratorHposUpdatesApi::new(&db_client).await?;

        let starting_request_info = HostUpdateRequest {
            channel: "nixos-unstable".to_string(),
            device_id: "test_device_id".to_string(),
        };

        // TODO: Remove this once we have a way to test the orchestrator api
        // Insert a test host and hoster (minimal, not strictly required for current logic)
        let host = create_test_host("test_device_id", None, None, None, None, None);
        let _host_id = api.host_collection.insert_one_into(host).await?;

        let update_request = HostUpdateRequest {
            channel: "nixos-unstable".to_string(),
            device_id: "test_device_id".to_string(),
        };
        let msg_payload = serde_json::to_vec(&update_request).unwrap();
        let msg =
            Arc::new(NatsMessage::new("HOST.orchestrator.update", msg_payload).into_message());

        let result = api.handle_host_update(msg).await?;

        assert!(result.maybe_response_tags.is_some());
        let tags = result.maybe_response_tags.unwrap();
        assert!(tags.contains_key(TAG_MAP_PREFIX_DESIGNATED_HOST));
        assert_eq!(tags[TAG_MAP_PREFIX_DESIGNATED_HOST], "test_device_id");

        let ending_request_info = result.info.request_info;
        assert_eq!(
            ending_request_info.device_id,
            starting_request_info.device_id
        );
        assert_eq!(ending_request_info.channel, starting_request_info.channel);
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_host_update_response() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");
        let api = OrchestratorHposUpdatesApi::new(&db_client).await?;

        // Insert a test host
        let mut host = create_test_host("test_device_id", None, None, None, None, None);
        host.device_id = "test_device_id".to_string();
        let _host_id = api.host_collection.insert_one_into(host).await?;

        let update_request = HostUpdateRequest {
            channel: "nixos-unstable".to_string(),
            device_id: "test_device_id".to_string(),
        };
        let response_info = HostUpdateInfo {
            request_info: update_request.clone(),
            state: HostUpdateState::Completed,
            context: Some("Update performed successfully".to_string()),
        };
        let update_result = HostUpdateApiRequest {
            info: response_info.clone(),
            maybe_response_tags: None,
            maybe_headers: None,
        };
        let msg_payload = serde_json::to_vec(&update_result).unwrap();
        let msg =
            Arc::new(NatsMessage::new("HOST.orchestrator.status", msg_payload).into_message());
        let r = api.handle_host_update_response(msg).await?;

        let ending_request_info = r.info.request_info;
        assert_eq!(ending_request_info.device_id, update_request.device_id);
        assert_eq!(ending_request_info.channel, update_request.channel);

        // Check that the host status was updated in the DB
        let raw_doc = db_client
            .database(DATABASE_NAME)
            .collection::<bson::Document>("host")
            .find_one(bson::doc! { "device_id": "test_device_id" })
            .await?
            .expect("Host should exist");

        println!("raw_doc >>>>>>>>>>> {:#?}", raw_doc);
        assert!(raw_doc.get("status").is_some());
        assert!(raw_doc.get("channel").is_some());
        assert_eq!(
            raw_doc.get("channel").unwrap().as_str().unwrap(),
            "nixos-unstable"
        );
        assert!(raw_doc.get("status").is_some());
        let status_bson = raw_doc.get("status").unwrap();
        let status: HostStatus = bson::from_bson(status_bson.clone()).unwrap();
        assert_eq!(
            status,
            HostStatus::Active("Update performed successfully".to_string())
        );

        Ok(())
    }
}
