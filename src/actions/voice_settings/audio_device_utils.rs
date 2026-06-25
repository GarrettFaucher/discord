use crate::protocol::Device;

use std::{collections::HashMap, sync::OnceLock};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AudioDeviceType {
	Input,
	Output,
}

impl AudioDeviceType {
	// Discord's volume scale: input is 0–100, output is 0–200.
	pub fn max_volume(&self) -> f32 {
		match self {
			Self::Input => 100.0,
			Self::Output => 200.0,
		}
	}
}

#[derive(Clone)]
pub struct AudioDeviceWrapper {
	pub device_id: String,
	pub volume: f32,
	pub available_devices: Vec<Device>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UserVoiceSettings {
	pub nick: String,
	pub volume: f32,
	pub mute: bool,
	pub self_mute: bool,
	pub self_deaf: bool,
	pub server_mute: bool,
	pub server_deaf: bool,
}

pub fn audio_input_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn audio_output_settings() -> &'static RwLock<Option<AudioDeviceWrapper>> {
	static SETTINGS: OnceLock<RwLock<Option<AudioDeviceWrapper>>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(None))
}

pub fn user_voice_settings_map() -> &'static RwLock<HashMap<String, UserVoiceSettings>> {
	static MAP: OnceLock<RwLock<HashMap<String, UserVoiceSettings>>> = OnceLock::new();
	MAP.get_or_init(Default::default)
}

pub async fn get_audio_device_settings(
	device_type: &AudioDeviceType,
) -> Option<AudioDeviceWrapper> {
	match device_type {
		AudioDeviceType::Input => audio_input_settings(),
		AudioDeviceType::Output => audio_output_settings(),
	}
	.read()
	.await
	.clone()
}
