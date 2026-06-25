pub mod audio_device_utils;
pub mod set_audio_device;
mod user_volume_control;
mod volume_control;

pub use set_audio_device::SetAudioDeviceAction;
pub use user_volume_control::UserVolumeControlAction;
pub use volume_control::VolumeControlAction;

use crate::protocol::ServerCommand;
use crate::ws_server::send_command;

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::Ordering::Relaxed;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};
use tokio::sync::RwLock;

// Last-known voice input mode ("PUSH_TO_TALK" | "VOICE_ACTIVITY"), updated via state feedback.
pub fn current_voice_mode() -> &'static RwLock<Option<String>> {
	static MODE: OnceLock<RwLock<Option<String>>> = OnceLock::new();
	MODE.get_or_init(|| RwLock::new(None))
}

// Send a command over the bridge, reflecting optimistic button state on success and alerting
// when no client is connected.
pub(crate) async fn send_with_state(
	instance: &Instance,
	command: ServerCommand,
	next_state: u16,
) -> OpenActionResult<()> {
	match send_command(command).await {
		Ok(()) => instance.set_state(next_state).await,
		Err(()) => instance.show_alert().await,
	}
}

pub struct ToggleMuteAction;
#[async_trait]
impl Action for ToggleMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglemute";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let new_mute = instance.current_state_index.load(Relaxed) == 0;
		send_with_state(
			instance,
			ServerCommand::SetMute { value: new_mute },
			if new_mute { 1 } else { 0 },
		)
		.await
	}
}

pub struct ToggleDeafenAction;
#[async_trait]
impl Action for ToggleDeafenAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.toggledeafen";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let new_deaf = instance.current_state_index.load(Relaxed) == 0;
		send_with_state(
			instance,
			ServerCommand::SetDeafen { value: new_deaf },
			if new_deaf { 1 } else { 0 },
		)
		.await
	}
}

pub struct PushToMuteAction;
#[async_trait]
impl Action for PushToMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtomute";
	type Settings = HashMap<String, String>;

	async fn key_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_with_state(instance, ServerCommand::SetMute { value: true }, 1).await
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_with_state(instance, ServerCommand::SetMute { value: false }, 0).await
	}
}

pub struct PushToTalkAction;
#[async_trait]
impl Action for PushToTalkAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtotalk";
	type Settings = HashMap<String, String>;

	async fn key_down(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_with_state(instance, ServerCommand::SetMute { value: false }, 1).await
	}

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_with_state(instance, ServerCommand::SetMute { value: true }, 0).await
	}
}

pub struct ToggleVoiceInputModeAction;
#[async_trait]
impl Action for ToggleVoiceInputModeAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglevoiceinputmode";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		let mode_lock = current_voice_mode().read().await;
		let Some(current_mode) = mode_lock.as_deref() else {
			log::error!("Voice mode not yet known");
			instance.show_alert().await?;
			return Ok(());
		};

		let is_ptt = current_mode == "PUSH_TO_TALK";
		let new_mode = if is_ptt {
			"VOICE_ACTIVITY"
		} else {
			"PUSH_TO_TALK"
		};
		drop(mode_lock);

		send_with_state(
			instance,
			ServerCommand::SetVoiceInputMode {
				mode: new_mode.to_owned(),
			},
			if is_ptt { 0 } else { 1 },
		)
		.await
	}
}
