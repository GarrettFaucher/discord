use crate::client::discord_client;

use std::sync::{
	LazyLock,
	atomic::{AtomicBool, Ordering::Relaxed},
};

use dashmap::DashMap;
use discord_ipc_rust::models::send::commands::{SentCommand, SetVoiceSettingsArgs};
use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait, set_global_settings, visible_instances};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackendMode {
	#[default]
	Discord,
	Equibop,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct ActionSettings {
	pub backend: BackendMode,
}

// Shared assumed equibop voice state — one source of truth regardless of how many buttons exist.
// Mute and deafen are tracked independently; the visual mute display is `mute || deafen` (deafen
// implies muted), but the underlying mute flag is preserved across deafen toggles.
pub static EQUIBOP_MIC_MUTED: AtomicBool = AtomicBool::new(false);
pub static EQUIBOP_DEAFENED: AtomicBool = AtomicBool::new(false);

fn visual_muted() -> bool {
	EQUIBOP_MIC_MUTED.load(Relaxed) || EQUIBOP_DEAFENED.load(Relaxed)
}

// Per-instance backend setting cache so key handlers always have the current backend
// even when the host sends empty settings in key events.
pub static SETTINGS_CACHE: LazyLock<DashMap<String, ActionSettings>> = LazyLock::new(DashMap::new);

fn cached_settings(instance: &Instance, fallback: &ActionSettings) -> ActionSettings {
	SETTINGS_CACHE
		.get(&instance.instance_id)
		.map(|e| e.clone())
		.unwrap_or_else(|| fallback.clone())
}

fn cache_update(instance: &Instance, settings: &ActionSettings) {
	SETTINGS_CACHE.insert(instance.instance_id.clone(), settings.clone());
}

fn cache_remove(instance: &Instance) {
	SETTINGS_CACHE.remove(&instance.instance_id);
}

fn is_equibop(instance: &Instance) -> bool {
	SETTINGS_CACHE
		.get(&instance.instance_id)
		.map(|s| s.backend == BackendMode::Equibop)
		.unwrap_or(false)
}

// Update all visible ToggleMute button visuals to match the current visual mute state
// (mute OR deafen — deafen always shows muted even though the underlying mute is preserved).
async fn sync_mute_visuals() {
	let muted = visual_muted();
	for instance in visible_instances(ToggleMuteAction::UUID).await {
		if is_equibop(&instance) {
			let _ = instance.set_state(if muted { 1 } else { 0 }).await;
		}
	}
}

// Update all visible ToggleDeafen button visuals to match the current global deafen state.
async fn sync_deafen_visuals() {
	let deafened = EQUIBOP_DEAFENED.load(Relaxed);
	for instance in visible_instances(ToggleDeafenAction::UUID).await {
		if is_equibop(&instance) {
			let _ = instance.set_state(if deafened { 1 } else { 0 }).await;
		}
	}
}

// Persist the current equibop state into global settings so the PI can read it
// and it survives plugin restarts.
async fn save_equibop_state() {
	let mut current = crate::current_settings().write().await;
	current.equibop_mic_muted = EQUIBOP_MIC_MUTED.load(Relaxed);
	current.equibop_deafened = EQUIBOP_DEAFENED.load(Relaxed);
	if let Err(e) = set_global_settings(&*current).await {
		log::error!("Failed to persist equibop state: {}", e);
	}
}

// Centralise the voice settings RPC call and Stream Deck feedback logic.
async fn update_voice_setting(
	instance: &Instance,
	args: SetVoiceSettingsArgs,
	next_state: usize,
) -> OpenActionResult<()> {
	let mut client_lock = discord_client().write().await;
	let Some(client) = client_lock.as_mut() else {
		log::error!("Discord client not initialized");
		instance.show_alert().await?;
		return Ok(());
	};

	match client
		.emit_command(&SentCommand::SetVoiceSettings(args))
		.await
	{
		Ok(_) => {
			instance.set_state(next_state as u16).await?;
		}
		Err(e) => {
			log::error!("Failed to update voice state: {}", e);
			instance.show_alert().await?;
		}
	}

	Ok(())
}

// Shared lifecycle helpers.
async fn on_appear(instance: &Instance, settings: &ActionSettings, is_deafen: bool) -> OpenActionResult<()> {
	cache_update(instance, settings);
	if settings.backend == BackendMode::Equibop {
		let state = if is_deafen { EQUIBOP_DEAFENED.load(Relaxed) } else { visual_muted() };
		instance.set_state(if state { 1 } else { 0 }).await?;
	}
	Ok(())
}

async fn on_settings_change(instance: &Instance, settings: &ActionSettings, is_deafen: bool) -> OpenActionResult<()> {
	cache_update(instance, settings);
	if settings.backend == BackendMode::Equibop {
		let state = if is_deafen { EQUIBOP_DEAFENED.load(Relaxed) } else { visual_muted() };
		instance.set_state(if state { 1 } else { 0 }).await?;
	}
	Ok(())
}

pub struct ToggleMuteAction;
#[async_trait]
impl Action for ToggleMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglemute";
	type Settings = ActionSettings;

	async fn will_appear(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_appear(instance, settings, false).await
	}

	async fn will_disappear(&self, instance: &Instance, _settings: &Self::Settings) -> OpenActionResult<()> {
		cache_remove(instance);
		Ok(())
	}

	async fn did_receive_settings(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_settings_change(instance, settings, false).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				use std::sync::atomic::Ordering::Relaxed;
				let current_state = instance.current_state_index.load(Relaxed);
				let new_mute = current_state == 0;
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { mute: Some(new_mute), ..Default::default() },
					if new_mute { 1 } else { 0 },
				)
				.await
			}
			BackendMode::Equibop => {
				// Visual mute is mute || deafen. Pressing MUTE always toggles the visual:
				//   - If currently muted (either flag set): clear BOTH and issue a CLI
				//     toggle for each flag that was previously set.
				//   - If currently unmuted: set mute and issue --toggle-mic.
				if visual_muted() {
					let was_muted = EQUIBOP_MIC_MUTED.swap(false, Relaxed);
					let was_deafened = EQUIBOP_DEAFENED.swap(false, Relaxed);
					if was_muted {
						crate::equibop::run_equibop("--toggle-mic").await;
					}
					if was_deafened {
						crate::equibop::run_equibop("--toggle-deafen").await;
					}
				} else {
					EQUIBOP_MIC_MUTED.store(true, Relaxed);
					crate::equibop::run_equibop("--toggle-mic").await;
				}
				sync_mute_visuals().await;
				sync_deafen_visuals().await;
				save_equibop_state().await;
				Ok(())
			}
		}
	}

	// Flip: correct the assumed mic state without running equibop.
	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		_payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		if current.backend == BackendMode::Equibop {
			let new_muted = !EQUIBOP_MIC_MUTED.load(Relaxed);
			EQUIBOP_MIC_MUTED.store(new_muted, Relaxed);
			sync_mute_visuals().await;
			save_equibop_state().await;
		}
		Ok(())
	}
}

pub struct ToggleDeafenAction;
#[async_trait]
impl Action for ToggleDeafenAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.toggledeafen";
	type Settings = ActionSettings;

	async fn will_appear(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_appear(instance, settings, true).await
	}

	async fn will_disappear(&self, instance: &Instance, _settings: &Self::Settings) -> OpenActionResult<()> {
		cache_remove(instance);
		Ok(())
	}

	async fn did_receive_settings(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_settings_change(instance, settings, true).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				use std::sync::atomic::Ordering::Relaxed;
				let current_state = instance.current_state_index.load(Relaxed);
				let new_deaf = current_state == 0;
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { deaf: Some(new_deaf), ..Default::default() },
					if new_deaf { 1 } else { 0 },
				)
				.await
			}
			BackendMode::Equibop => {
				// Toggle deafen only; underlying mute is preserved (mute appears T while
				// deafened thanks to visual_muted, but its stored value never changes here).
				let new_deafened = !EQUIBOP_DEAFENED.load(Relaxed);
				EQUIBOP_DEAFENED.store(new_deafened, Relaxed);
				crate::equibop::run_equibop("--toggle-deafen").await;
				sync_mute_visuals().await;
				sync_deafen_visuals().await;
				save_equibop_state().await;
				Ok(())
			}
		}
	}

	// Flip: correct the assumed deafen state without running equibop.
	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		_payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		if current.backend == BackendMode::Equibop {
			let new_deafened = !EQUIBOP_DEAFENED.load(Relaxed);
			EQUIBOP_DEAFENED.store(new_deafened, Relaxed);
			sync_mute_visuals().await;
			sync_deafen_visuals().await;
			save_equibop_state().await;
		}
		Ok(())
	}
}

pub struct PushToMuteAction;
#[async_trait]
impl Action for PushToMuteAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtomute";
	type Settings = ActionSettings;

	async fn will_appear(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_appear(instance, settings, false).await
	}

	async fn will_disappear(&self, instance: &Instance, _settings: &Self::Settings) -> OpenActionResult<()> {
		cache_remove(instance);
		Ok(())
	}

	async fn did_receive_settings(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_settings_change(instance, settings, false).await
	}

	async fn key_down(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { mute: Some(true), ..Default::default() },
					1,
				)
				.await
			}
			BackendMode::Equibop => {
				if !EQUIBOP_DEAFENED.load(Relaxed) && !EQUIBOP_MIC_MUTED.load(Relaxed) {
					EQUIBOP_MIC_MUTED.store(true, Relaxed);
					crate::equibop::run_equibop("--toggle-mic").await;
					sync_mute_visuals().await;
					save_equibop_state().await;
				}
				instance.set_state(1).await
			}
		}
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { mute: Some(false), ..Default::default() },
					0,
				)
				.await
			}
			BackendMode::Equibop => {
				if !EQUIBOP_DEAFENED.load(Relaxed) && EQUIBOP_MIC_MUTED.load(Relaxed) {
					EQUIBOP_MIC_MUTED.store(false, Relaxed);
					crate::equibop::run_equibop("--toggle-mic").await;
					sync_mute_visuals().await;
					save_equibop_state().await;
				}
				instance.set_state(0).await
			}
		}
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		_payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		if current.backend == BackendMode::Equibop {
			let new_muted = !EQUIBOP_MIC_MUTED.load(Relaxed);
			EQUIBOP_MIC_MUTED.store(new_muted, Relaxed);
			sync_mute_visuals().await;
			save_equibop_state().await;
		}
		Ok(())
	}
}

pub struct PushToTalkAction;
#[async_trait]
impl Action for PushToTalkAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.pushtotalk";
	type Settings = ActionSettings;

	async fn will_appear(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_appear(instance, settings, false).await
	}

	async fn will_disappear(&self, instance: &Instance, _settings: &Self::Settings) -> OpenActionResult<()> {
		cache_remove(instance);
		Ok(())
	}

	async fn did_receive_settings(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		on_settings_change(instance, settings, false).await
	}

	async fn key_down(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { mute: Some(false), ..Default::default() },
					1,
				)
				.await
			}
			BackendMode::Equibop => {
				if !EQUIBOP_DEAFENED.load(Relaxed) && EQUIBOP_MIC_MUTED.load(Relaxed) {
					EQUIBOP_MIC_MUTED.store(false, Relaxed);
					crate::equibop::run_equibop("--toggle-mic").await;
					sync_mute_visuals().await;
					save_equibop_state().await;
				}
				instance.set_state(1).await
			}
		}
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		match current.backend {
			BackendMode::Discord => {
				update_voice_setting(
					instance,
					SetVoiceSettingsArgs { mute: Some(true), ..Default::default() },
					0,
				)
				.await
			}
			BackendMode::Equibop => {
				if !EQUIBOP_DEAFENED.load(Relaxed) && !EQUIBOP_MIC_MUTED.load(Relaxed) {
					EQUIBOP_MIC_MUTED.store(true, Relaxed);
					crate::equibop::run_equibop("--toggle-mic").await;
					sync_mute_visuals().await;
					save_equibop_state().await;
				}
				instance.set_state(0).await
			}
		}
	}

	async fn send_to_plugin(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		_payload: &serde_json::Value,
	) -> OpenActionResult<()> {
		let current = cached_settings(instance, settings);
		if current.backend == BackendMode::Equibop {
			let new_muted = !EQUIBOP_MIC_MUTED.load(Relaxed);
			EQUIBOP_MIC_MUTED.store(new_muted, Relaxed);
			sync_mute_visuals().await;
			save_equibop_state().await;
		}
		Ok(())
	}
}
