//! Shared runtime state populated by inbound client messages and read by action handlers.

use std::sync::OnceLock;

use tokio::sync::RwLock;

/// The voice channel the user is currently connected to, or `None` when not in voice.
/// Updated from each `stateUpdate`; used by the Voice Channel action for join/leave toggling
/// and button feedback.
pub fn current_voice_channel() -> &'static RwLock<Option<String>> {
	static CHANNEL: OnceLock<RwLock<Option<String>>> = OnceLock::new();
	CHANNEL.get_or_init(|| RwLock::new(None))
}
