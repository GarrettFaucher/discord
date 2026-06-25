use crate::cache::notification_cache;
use crate::protocol::ServerCommand;
use crate::ws_server::send_command;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

pub async fn update_title(instance: &Instance) -> OpenActionResult<()> {
	let cache = notification_cache().read().await;
	let title = format!("{}", cache.len());

	if let Err(e) = instance.set_title(Some(title), None).await {
		log::error!("Failed to update notifications action title: {}", e);
		instance.show_alert().await?;
	}

	Ok(())
}

#[derive(Serialize, Deserialize, Default)]
pub enum NotificationsActionType {
	#[default]
	DoNothing,
	Clear,
	OpenAndClear,
	CycleRecentFirst,
	CycleOldestFirst,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NotificationsSettings {
	pub action_type: NotificationsActionType,
}

pub struct NotificationsAction;
#[async_trait]
impl Action for NotificationsAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.notifications";
	type Settings = NotificationsSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		update_title(instance).await
	}

	async fn key_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let notification = match settings.action_type {
			NotificationsActionType::DoNothing => return Ok(()),
			NotificationsActionType::Clear => {
				notification_cache().write().await.clear();
				update_title(instance).await?;
				return Ok(());
			}
			NotificationsActionType::OpenAndClear => {
				let mut cache = notification_cache().write().await;
				let notification = cache.pop_back();
				cache.clear();
				notification
			}
			NotificationsActionType::CycleRecentFirst => {
				notification_cache().write().await.pop_back()
			}
			NotificationsActionType::CycleOldestFirst => {
				notification_cache().write().await.pop_front()
			}
		};

		let Some(notification) = notification else {
			instance.show_alert().await?;
			return Ok(());
		};

		update_title(instance).await?;

		// The client resolves the guild from the channel, so `guild_id` is left unset.
		if send_command(ServerCommand::SelectTextChannel {
			guild_id: None,
			channel_id: notification.channel_id,
		})
		.await
		.is_err()
		{
			instance.show_alert().await?;
		}

		Ok(())
	}
}
