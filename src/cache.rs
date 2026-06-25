use crate::protocol::{Channel, Guild, ServerCommand, Sound};
use crate::ws_server::send_command;

use std::collections::VecDeque;
use std::sync::OnceLock;

use openaction::{Instance, OpenActionResult};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// A guild with its selectable voice/text channels, as sent to the property inspector.
#[derive(Serialize, Clone)]
pub struct CachedGuild {
	pub id: String,
	pub name: String,
	pub voice: Vec<Channel>,
	pub text: Vec<Channel>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CachedSoundboardSound {
	pub name: String,
	pub guild_id: String,
	pub sound_id: String,
	pub emoji_name: Option<String>,
}

pub fn guild_cache() -> &'static RwLock<Vec<CachedGuild>> {
	static CACHE: OnceLock<RwLock<Vec<CachedGuild>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn soundboard_sounds_cache() -> &'static RwLock<Vec<CachedSoundboardSound>> {
	static CACHE: OnceLock<RwLock<Vec<CachedSoundboardSound>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(Vec::new()))
}

#[derive(Serialize, Clone)]
pub struct CachedNotification {
	pub channel_id: String,
}

pub fn notification_cache() -> &'static RwLock<VecDeque<CachedNotification>> {
	static CACHE: OnceLock<RwLock<VecDeque<CachedNotification>>> = OnceLock::new();
	CACHE.get_or_init(|| RwLock::new(VecDeque::new()))
}

pub async fn update_guild_cache(guilds: Vec<Guild>) {
	let mut cached: Vec<CachedGuild> = guilds
		.into_iter()
		.map(|g| CachedGuild {
			id: g.id,
			name: g.name,
			voice: g.voice,
			text: g.text,
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*guild_cache().write().await = cached;
}

// Ask the client to (re)send the guild list; the reply repopulates the cache.
pub async fn refresh_guild_cache(instance: &Instance) -> OpenActionResult<()> {
	if send_command(ServerCommand::RequestGuilds).await.is_err() {
		instance.show_alert().await?;
	}
	Ok(())
}

pub async fn update_soundboard_cache(sounds: Vec<Sound>) {
	let mut cached: Vec<CachedSoundboardSound> = sounds
		.into_iter()
		.map(|s| CachedSoundboardSound {
			name: s.name,
			guild_id: s.guild_id,
			sound_id: s.sound_id,
			emoji_name: s.emoji_name,
		})
		.collect();
	cached.sort_by_key(|x| x.name.to_lowercase());
	*soundboard_sounds_cache().write().await = cached;
}

pub async fn refresh_soundboard_cache(instance: &Instance) -> OpenActionResult<()> {
	if send_command(ServerCommand::RequestSoundboard).await.is_err() {
		instance.show_alert().await?;
	}
	Ok(())
}

pub async fn add_notification_to_cache(channel_id: String) {
	notification_cache()
		.write()
		.await
		.push_back(CachedNotification { channel_id });
}
