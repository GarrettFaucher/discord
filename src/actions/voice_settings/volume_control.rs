use super::audio_device_utils::{AudioDeviceType, get_audio_device_settings};
use super::send_with_state;

use crate::protocol::ServerCommand;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub enum VolumeControlActionType {
	#[default]
	Increase,
	Decrease,
	Set,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct VolumeControlSettings {
	pub device_type: AudioDeviceType,
	pub action_type: VolumeControlActionType,
	pub step_size: u8,
	pub set_volume: u8,
}

impl Default for VolumeControlSettings {
	fn default() -> Self {
		Self {
			device_type: AudioDeviceType::Input,
			action_type: VolumeControlActionType::default(),
			step_size: 5,
			set_volume: 100,
		}
	}
}

async fn adjust_volume(
	instance: &Instance,
	device_type: &AudioDeviceType,
	value: f32,
	set: bool,
) -> OpenActionResult<()> {
	let Some(device_settings) = get_audio_device_settings(device_type).await else {
		log::error!(
			"Failed to obtain voice settings for {:?} device",
			device_type
		);
		instance.show_alert().await?;
		return Ok(());
	};

	let current = device_settings.volume;
	let new_volume = if set { value } else { current + value }.clamp(0.0, device_type.max_volume());

	if new_volume == current {
		return Ok(());
	}

	let command = match device_type {
		AudioDeviceType::Input => ServerCommand::SetInputVolume { value: new_volume },
		AudioDeviceType::Output => ServerCommand::SetOutputVolume { value: new_volume },
	};

	send_with_state(instance, command, 0).await
}

pub struct VolumeControlAction;
#[async_trait]
impl Action for VolumeControlAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.volumecontrol";
	type Settings = VolumeControlSettings;

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let value = match settings.action_type {
			VolumeControlActionType::Increase => settings.step_size as f32,
			VolumeControlActionType::Decrease => -(settings.step_size as f32),
			VolumeControlActionType::Set => settings.set_volume as f32,
		};

		adjust_volume(
			instance,
			&settings.device_type,
			value,
			matches!(settings.action_type, VolumeControlActionType::Set),
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

		adjust_volume(instance, &settings.device_type, delta, false).await
	}
}
