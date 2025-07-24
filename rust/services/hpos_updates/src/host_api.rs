use super::{
    types::{HostUpdateApiResult, HostUpdateRequest, HostUpdateResponseInfo, HostUpdateResult},
    HostUpdatesServiceApi, TAG_MAP_PREFIX_DESIGNATED_HOST,
};
use anyhow::Result;
use async_nats::Message;
use bson::{self, doc};
use nats_utils::types::ServiceError;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

#[derive(Clone, Debug)]
pub struct HostUpdatesApi {}

impl HostUpdatesServiceApi for HostUpdatesApi {}

impl HostUpdatesApi {
    pub async fn handle_host_update_command(&self, msg: Arc<Message>) -> Result<HostUpdateApiResult, ServiceError>  {
    let host_command = Self::convert_msg_to_type::<HostUpdateResult>(msg)?;

        let r = match host_command {
            HostUpdateResult::Success(info) => {
                let channel = info.request_info.channel.clone();
                let device_id = info.request_info.device_id.clone();

                // handle update here
                self.nixos_channel_update(channel.clone(), device_id.clone());

                HostUpdateResult::Success(HostUpdateResponseInfo {
                    info: format!("Successfully added the Nixos channel to {} on device {}", channel, device_id),
                    request_info: info.request_info
                })
            }
            HostUpdateResult::Error(e)  => HostUpdateResult::Error(e)
        };

        Ok(HostUpdateApiResult {
            result: r, 
            maybe_response_tags: None,
            maybe_headers: None
        })

    }

    async fn nixos_channel_update(&self, channel: String, device_id: String) {}
}
