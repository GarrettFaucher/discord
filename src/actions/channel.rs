use crate::cache::{CachedGuild, guild_cache, refresh_guild_cache};
use crate::protocol::ServerCommand;
use crate::state::current_voice_channel;
use crate::ws_server::send_command;

use std::sync::Arc;

use openaction::{
	Action, ActionUuid, Instance, OpenActionResult, async_trait, visible_instances,
};
use serde::{Deserialize, Serialize};

async fn get_all_instances() -> impl Iterator<Item = Arc<Instance>> {
	visible_instances(TextChannelAction::UUID)
		.await
		.into_iter()
		.chain(visible_instances(VoiceChannelAction::UUID).await)
		.chain(visible_instances(crate::actions::SoundboardAction::UUID).await)
}

pub async fn send_guilds_to_pi(instance: Option<&Instance>) {
	#[derive(Serialize)]
	struct Payload {
		guilds: Vec<CachedGuild>,
	}

	let cache = guild_cache().read().await;
	let payload = Payload {
		guilds: cache.clone(),
	};

	match instance {
		Some(inst) => {
			let _ = inst.send_to_property_inspector(&payload).await;
		}
		None => {
			for inst in get_all_instances().await {
				let _ = inst.send_to_property_inspector(&payload).await;
			}
		}
	}
}

pub async fn send_cached_guilds_to_pi(instance: &Instance) -> OpenActionResult<()> {
	if !guild_cache().read().await.is_empty() {
		send_guilds_to_pi(Some(instance)).await;
		Ok(())
	} else {
		refresh_guild_cache(instance).await
	}
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChannelActionSettings {
	pub guild_id: String,
	pub channel_id: String,
}

pub struct TextChannelAction;
#[async_trait]
impl Action for TextChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.textchannel";
	type Settings = ChannelActionSettings;

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_cached_guilds_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		if settings.channel_id.is_empty() {
			instance.show_alert().await?;
			return Ok(());
		}

		let guild_id = (!settings.guild_id.is_empty()).then(|| settings.guild_id.clone());

		if send_command(ServerCommand::SelectTextChannel {
			guild_id,
			channel_id: settings.channel_id.clone(),
		})
		.await
		.is_err()
		{
			instance.show_alert().await?;
		}

		Ok(())
	}
}

async fn sync_voice_channel_state(
	instance: &Instance,
	settings: &ChannelActionSettings,
) -> OpenActionResult<()> {
	let is_active = current_voice_channel()
		.read()
		.await
		.as_deref()
		.is_some_and(|ch| settings.channel_id == ch);

	instance.set_state(if is_active { 1 } else { 0 }).await
}

pub struct VoiceChannelAction;
#[async_trait]
impl Action for VoiceChannelAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.voicechannel";
	type Settings = ChannelActionSettings;

	async fn will_appear(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		sync_voice_channel_state(instance, settings).await
	}

	async fn did_receive_settings(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		sync_voice_channel_state(instance, settings).await
	}

	async fn property_inspector_did_appear(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		send_cached_guilds_to_pi(instance).await
	}

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		if settings.channel_id.is_empty() {
			instance.show_alert().await?;
			return Ok(());
		}

		// Toggle: leave if already in this channel, otherwise join it.
		let current = current_voice_channel().read().await;
		let target = if current.as_deref() != Some(settings.channel_id.as_str()) {
			Some(settings.channel_id.clone())
		} else {
			None
		};
		drop(current);

		if send_command(ServerCommand::SelectVoiceChannel { channel_id: target })
			.await
			.is_err()
		{
			instance.show_alert().await?;
		}

		Ok(())
	}
}
