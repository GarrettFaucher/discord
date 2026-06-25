mod actions;
mod cache;
mod feedback;
mod protocol;
mod state;
mod ws_server;

use actions::*;

use std::sync::OnceLock;

use openaction::{
	OpenActionResult, async_trait, get_global_settings, global_events, register_action, run,
	set_global_settings,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

const DEFAULT_PORT: u16 = 6789;

fn default_port() -> u16 {
	DEFAULT_PORT
}

// Persisted plugin configuration. The only user-facing setting is the bridge port; `error`
// surfaces bind/connection problems in the property inspector.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
	#[serde(default = "default_port")]
	pub port: u16,
	pub error: Option<String>,
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			port: DEFAULT_PORT,
			error: None,
		}
	}
}

// Global storage for the last-applied settings so every module can read them.
pub fn current_settings() -> &'static RwLock<Settings> {
	static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();
	SETTINGS.get_or_init(|| RwLock::new(Settings::default()))
}

// Handle to the running bridge server task, so a port change can restart it.
fn server_handle() -> &'static RwLock<Option<JoinHandle<()>>> {
	static HANDLE: OnceLock<RwLock<Option<JoinHandle<()>>>> = OnceLock::new();
	HANDLE.get_or_init(|| RwLock::new(None))
}

// (Re)bind the bridge server on the given port, aborting any previous instance.
async fn restart_server(port: u16) {
	let bind_result = {
		let mut handle = server_handle().write().await;

		// Abort the previous server AND wait for it to finish so its TcpListener is dropped before
		// we rebind — `abort()` only schedules cancellation, so binding without this await can race
		// the old listener and fail with EADDRINUSE, leaving the bridge silently dead.
		if let Some(old) = handle.take() {
			old.abort();
			let _ = old.await;
		}

		match TcpListener::bind(("127.0.0.1", port)).await {
			Ok(listener) => {
				*handle = Some(tokio::spawn(ws_server::serve(listener)));
				Ok(())
			}
			Err(e) => Err(e),
		}
	};

	match bind_result {
		Ok(()) => clear_bridge_error().await,
		Err(e) => {
			log::error!("Failed to bind Equibop bridge on 127.0.0.1:{port}: {e}");
			set_bridge_error(format!("Could not bind the bridge on port {port}: {e}")).await;
		}
	}
}

// Surface a bridge error in the global settings so the property inspector can show it.
async fn set_bridge_error(message: String) {
	let snapshot = {
		let mut settings = current_settings().write().await;
		if settings.error.as_deref() == Some(message.as_str()) {
			return;
		}
		settings.error = Some(message);
		settings.clone()
	};
	if let Err(e) = set_global_settings(&snapshot).await {
		log::error!("Failed to persist bridge error: {e}");
	}
}

// Clear a previously reported bridge error once the server binds successfully.
async fn clear_bridge_error() {
	let snapshot = {
		let mut settings = current_settings().write().await;
		if settings.error.is_none() {
			return;
		}
		settings.error = None;
		settings.clone()
	};
	if let Err(e) = set_global_settings(&snapshot).await {
		log::error!("Failed to clear bridge error: {e}");
	}
}

// Handles global setting updates pushed from the Stream Deck host.
pub struct GlobalEventHandler;
#[async_trait]
impl global_events::GlobalEventHandler for GlobalEventHandler {
	async fn plugin_ready(&self) -> OpenActionResult<()> {
		get_global_settings().await
	}

	async fn did_receive_global_settings(
		&self,
		event: global_events::DidReceiveGlobalSettingsEvent,
	) -> OpenActionResult<()> {
		let settings: Settings =
			serde_json::from_value(event.payload.settings).unwrap_or_default();

		let old_port = current_settings().read().await.port;
		let port_changed = settings.port != old_port;

		*current_settings().write().await = settings.clone();

		if port_changed {
			log::info!("Bridge port changed to {}, restarting server", settings.port);
			restart_server(settings.port).await;
		}

		Ok(())
	}
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	{
		use simplelog::*;
		if let Err(error) = TermLogger::init(
			LevelFilter::Debug,
			Config::default(),
			TerminalMode::Stdout,
			ColorChoice::Never,
		) {
			eprintln!("Logger initialization failed: {}", error);
		}
	}

	global_events::set_global_event_handler(&GlobalEventHandler);
	register_action(ToggleMuteAction).await;
	register_action(ToggleDeafenAction).await;
	register_action(PushToMuteAction).await;
	register_action(PushToTalkAction).await;
	register_action(ToggleVoiceInputModeAction).await;
	register_action(ToggleVideoAction).await;
	register_action(ToggleScreenshareAction).await;
	register_action(VolumeControlAction).await;
	register_action(UserVolumeControlAction).await;
	register_action(SetAudioDeviceAction).await;
	register_action(TextChannelAction).await;
	register_action(VoiceChannelAction).await;
	register_action(SoundboardAction).await;
	register_action(NotificationsAction).await;

	// Start the bridge on the default port before handing control to the OpenDeck event loop;
	// a different stored port arrives via `did_receive_global_settings` and restarts it.
	restart_server(current_settings().read().await.port).await;

	run(std::env::args().collect()).await
}
