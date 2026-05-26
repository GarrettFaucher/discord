use crate::client::discord_client;

use std::collections::HashMap;

use discord_ipc_rust::models::send::commands::SentCommand;
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};

pub struct ToggleVideoAction;

#[async_trait]
impl Action for ToggleVideoAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglevideo";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mut client_lock = discord_client().write().await;
		let Some(client) = client_lock.as_mut() else {
			log::error!("Discord client not initialized");
			instance.show_alert().await?;
			return Ok(());
		};

		if let Err(e) = client.emit_command(&SentCommand::ToggleVideo).await {
			log::error!("Failed to toggle video: {}", e);
			instance.show_alert().await?;
		}

		Ok(())
	}
}
