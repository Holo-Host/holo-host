use super::{
    types::{HostUpdateApiRequest, HostUpdateInfo, HostUpdateState},
    utils, HposUpdatesServiceApi,
};
use async_nats::Message;
use nats_utils::types::ServiceError;
use std::{fmt::Debug, sync::Arc};

#[derive(Clone, Debug)]
pub struct HostUpdatesApi {}

impl HposUpdatesServiceApi for HostUpdatesApi {}

impl HostUpdatesApi {
    pub async fn handle_host_update_command(
        &self,
        msg: Arc<Message>,
    ) -> Result<HostUpdateApiRequest, ServiceError> {
        let host_update_info = Self::convert_msg_to_type::<HostUpdateInfo>(msg)?;

        let info: HostUpdateInfo = match host_update_info.state {
            HostUpdateState::Pending => {
                let channel = host_update_info.request_info.channel.clone();
                let device_id = host_update_info.request_info.device_id.clone();

                log::info!("Processing NixOS channel update request: channel={channel}, device_id={device_id}");

                // Perform NixOS channel switch and rebuild
                match self.update_nixos_channel(&channel, &device_id).await {
                    Ok(_) => HostUpdateInfo {
                        request_info: host_update_info.request_info,
                        state: HostUpdateState::Completed,
                        context: Some(format!(
                            "Successfully updated NixOS channel to {} on device {}",
                            channel, device_id
                        )),
                    },
                    Err(e) => HostUpdateInfo {
                        request_info: host_update_info.request_info,
                        state: HostUpdateState::Failed,
                        context: Some(format!("Failed to update NixOS channel: {}", e)),
                    },
                }
            }
            HostUpdateState::Completed | HostUpdateState::Failed => {
                log::warn!(
                    "Host Agent received unexpected state in hpos update request. Ignoring hpos update request. host_update_info={host_update_info:?}"
                );
                host_update_info
            }
        };

        Ok(HostUpdateApiRequest {
            info,
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }

    // Draft for update (using bash :|)
    async fn update_nixos_channel(
        &self,
        channel: &str,
        device_id: &str,
    ) -> Result<(), ServiceError> {
        log::info!(
            "Starting NixOS channel update for device {} to channel {}",
            device_id,
            channel
        );

        // TODO: Add a check to see if the channel is already the current channel before attempting to switch/rebuild
        // TODO: Add a check to see if the channel is a valid channel

        // Step 1: Switch to the new channel
        let switch_cmd = format!(
            "nix-channel --add https://nixos.org/channels/nixos-{} nixos",
            channel
        );

        log::debug!("Executing channel switch command: {}", switch_cmd);
        utils::bash(&switch_cmd).await.map_err(|e| {
            ServiceError::internal(format!("Failed to switch NixOS channel: {}", e), None)
        })?;

        // Step 2: Update the channel
        let update_cmd = "nix-channel --update";
        log::debug!("Executing channel update command: {}", update_cmd);
        utils::bash(update_cmd).await.map_err(|e| {
            ServiceError::internal(format!("Failed to update NixOS channel: {}", e), None)
        })?;

        // Step 3: Rebuild the system
        let rebuild_cmd = "nixos-rebuild switch";
        log::debug!("Executing system rebuild command: {}", rebuild_cmd);
        utils::bash(rebuild_cmd).await.map_err(|e| {
            ServiceError::internal(format!("Failed to rebuild NixOS system: {}", e), None)
        })?;

        log::info!(
            "Successfully completed NixOS channel update for device {} to channel {}",
            device_id,
            channel
        );
        Ok(())
    }
}
