//! Applies inbound client feedback (state/devices/guilds/soundboard/notifications) to Stream
//! Deck button states and property inspectors. This replaces the old Discord-RPC event handler.

use crate::actions;
use crate::actions::voice_settings::audio_device_utils::{
	AudioDeviceWrapper, audio_input_settings, audio_output_settings, user_voice_settings_map,
};
use crate::cache;
use crate::protocol::{Devices, Guild, Sound};
use crate::state::current_voice_channel;

use openaction::{Action as _, ActionUuid, visible_instances};

/// Apply a full self voice/video state snapshot to every relevant button.
pub async fn apply_state_update(
	mute: bool,
	deaf: bool,
	input_mode: Option<String>,
	video: bool,
	screenshare: bool,
	channel_id: Option<String>,
) {
	// Deafening implies muted, so the mute button reflects either.
	update_action_state(actions::ToggleMuteAction::UUID, mute || deaf).await;
	update_action_state(actions::ToggleDeafenAction::UUID, deaf).await;

	if let Some(mode) = input_mode {
		let is_ptt = mode == "PUSH_TO_TALK";
		update_action_state(actions::ToggleVoiceInputModeAction::UUID, is_ptt).await;
		*actions::current_voice_mode().write().await = Some(mode);
	}

	update_action_state(actions::ToggleVideoAction::UUID, video).await;
	update_action_state(actions::ToggleScreenshareAction::UUID, screenshare).await;

	apply_voice_channel(channel_id).await;
}

async fn apply_voice_channel(channel_id: Option<String>) {
	if *current_voice_channel().read().await == channel_id {
		return;
	}

	*current_voice_channel().write().await = channel_id;

	// Refresh Voice Channel buttons so the active channel highlights correctly.
	for instance in visible_instances(actions::VoiceChannelAction::UUID).await {
		let _ = instance.get_settings().await;
	}
}

/// Populate the cached audio device lists/volumes and refresh the Set Audio Device PIs.
pub async fn apply_devices(devices: Devices) {
	*audio_input_settings().write().await = Some(AudioDeviceWrapper {
		device_id: devices.current_input,
		volume: devices.input_volume,
		available_devices: devices.input,
	});
	*audio_output_settings().write().await = Some(AudioDeviceWrapper {
		device_id: devices.current_output,
		volume: devices.output_volume,
		available_devices: devices.output,
	});

	for instance in visible_instances(actions::SetAudioDeviceAction::UUID).await {
		let _ = actions::voice_settings::set_audio_device::send_available_devices_to_pi(&instance)
			.await;
	}
}

/// Cache the guild/channel list and push it to any open channel/soundboard PIs.
pub async fn apply_guilds(guilds: Vec<Guild>) {
	cache::update_guild_cache(guilds).await;
	actions::channel::send_guilds_to_pi(None).await;
}

/// Cache the soundboard list and push it to any open Soundboard PIs.
pub async fn apply_soundboard(sounds: Vec<Sound>) {
	cache::update_soundboard_cache(sounds).await;
	actions::soundboard::send_sounds_to_pi(None).await;
}

/// Record an incoming notification and refresh the Notifications button counter.
pub async fn apply_notification(channel_id: String, _title: Option<String>, _body: Option<String>) {
	cache::add_notification_to_cache(channel_id).await;
	for instance in visible_instances(actions::NotificationsAction::UUID).await {
		let _ = actions::notifications::update_title(&instance).await;
	}
}

/// Clear cached, client-specific state when the Equibop client goes away.
pub async fn on_client_disconnected() {
	cache::guild_cache().write().await.clear();
	cache::soundboard_sounds_cache().write().await.clear();
	*current_voice_channel().write().await = None;
	user_voice_settings_map().write().await.clear();
	*audio_input_settings().write().await = None;
	*audio_output_settings().write().await = None;
}

async fn update_action_state(action_uuid: ActionUuid, active: bool) {
	let state = if active { 1 } else { 0 };
	for instance in visible_instances(action_uuid).await {
		if let Err(e) = instance.set_state(state).await {
			log::error!("Failed to update state for {action_uuid}: {e}");
		}
	}
}
