use super::audio_device_utils::{AudioDeviceType, user_voice_settings_map};

use crate::protocol::ServerCommand;
use crate::ws_server::send_command;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub enum UserVolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
	Mute,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct UserVolumeControlSettings {
	pub action_type: UserVolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
	pub user_id: Option<String>,
}

impl Default for UserVolumeControlSettings {
	fn default() -> Self {
		Self {
			action_type: UserVolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
			user_id: None,
		}
	}
}

async fn adjust_user_volume(
	instance: &Instance,
	user_id: String,
	value: f32,
	set: bool,
) -> OpenActionResult<()> {
	let max_volume = AudioDeviceType::Output.max_volume();

	let current_volume = match user_voice_settings_map().read().await.get(&user_id) {
		Some(settings) => settings.volume,
		None => {
			log::error!(
				"Failed to adjust volume for user '{}': user not found in voice settings map",
				user_id
			);
			instance.show_alert().await?;
			return Ok(());
		}
	};

	let new_volume = if set { value } else { current_volume + value }.clamp(0.0, max_volume);

	if new_volume == current_volume {
		return Ok(());
	}

	if send_command(ServerCommand::SetUserVolume {
		user_id,
		value: new_volume,
	})
	.await
	.is_err()
	{
		instance.show_alert().await?;
	}

	Ok(())
}

async fn send_users_to_pi(instance: &Instance) -> OpenActionResult<()> {
	#[derive(Serialize)]
	struct MinimalUser {
		pub id: String,
		pub nick: String,
	}

	#[derive(Serialize)]
	struct Payload {
		users: Vec<MinimalUser>,
	}

	let users = user_voice_settings_map()
		.read()
		.await
		.iter()
		.map(|(user_id, settings)| MinimalUser {
			id: user_id.clone(),
			nick: settings.nick.clone(),
		})
		.collect();

	instance
		.send_to_property_inspector(Payload { users })
		.await?;

	Ok(())
}

pub struct UserVolumeControlAction;
#[async_trait]
impl Action for UserVolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.uservolumecontrol";
	type Settings = UserVolumeControlSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_users_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let Some(user_id) = settings.user_id.as_ref() else {
			log::error!("Failed to update user voice settings: no user ID provided");
			instance.show_alert().await?;
			return Ok(());
		};

		if matches!(settings.action_type, UserVolumeControlActionType::Mute) {
			// TODO(T6.3): the bridge protocol has no per-user mute command yet. Surface an alert
			// until `setUserMute` is added alongside the userplugin handler.
			log::warn!("Per-user mute is not yet supported over the Equibop bridge");
			instance.show_alert().await?;
			return Ok(());
		}

		let value = match settings.action_type {
			UserVolumeControlActionType::Increase => settings.step_size as f32,
			UserVolumeControlActionType::Decrease => -(settings.step_size as f32),
			UserVolumeControlActionType::Set => settings.set_volume as f32,
			UserVolumeControlActionType::Mute => unreachable!(),
		};

		adjust_user_volume(
			instance,
			user_id.clone(),
			value,
			matches!(settings.action_type, UserVolumeControlActionType::Set),
		)
		.await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let delta = (settings.step_size as f32) * ticks as f32;

		if let Some(user_id) = &settings.user_id {
			adjust_user_volume(instance, user_id.clone(), delta, false).await
		} else {
			log::error!("Failed to adjust user volume: no user ID provided");
			instance.show_alert().await?;
			Ok(())
		}
	}
}
