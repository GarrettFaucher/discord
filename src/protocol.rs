//! The shared bridge protocol between this plugin (the WebSocket **server**) and the
//! Equibop userplugin (the **client**). Keep this in sync with `PROTOCOL.md` and the
//! userplugin repo; bump the version there if the wire format changes.
//!
//! Transport is text JSON frames. Every message carries a `type` discriminator
//! (serde internal tag). Unknown inbound `type`s are tolerated via [`ClientMessage::Unknown`].

use serde::{Deserialize, Serialize};

/// An audio input/output device as reported by the client.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Device {
	pub id: String,
	pub name: String,
}

/// A text or voice channel within a guild.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Channel {
	pub id: String,
	pub name: String,
}

/// A guild with its selectable voice and text channels already resolved.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Guild {
	pub id: String,
	pub name: String,
	#[serde(default)]
	pub voice: Vec<Channel>,
	#[serde(default)]
	pub text: Vec<Channel>,
}

/// A soundboard sound playable in the active call.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sound {
	pub guild_id: String,
	pub sound_id: String,
	pub name: String,
	#[serde(default)]
	pub emoji_name: Option<String>,
}

/// The current audio device lists, selections and volumes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Devices {
	#[serde(default)]
	pub input: Vec<Device>,
	#[serde(default)]
	pub output: Vec<Device>,
	#[serde(default)]
	pub current_input: String,
	#[serde(default)]
	pub current_output: String,
	#[serde(default)]
	pub input_volume: f32,
	#[serde(default)]
	pub output_volume: f32,
}

/// Commands the server (this plugin) sends to the client (Equibop userplugin).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerCommand {
	/// Handshake reply to the client's `hello`.
	Ready,
	/// Ask the client to (re)send the matching event.
	RequestState,
	RequestDevices,
	RequestGuilds,
	RequestSoundboard,
	/// Absolute self mute/deafen (used by Push-to-Mute / Push-to-Talk and the toggles).
	SetMute { value: bool },
	SetDeafen { value: bool },
	/// Flip current self mute/deafen client-side.
	ToggleMute,
	ToggleDeafen,
	/// `"PUSH_TO_TALK"` or `"VOICE_ACTIVITY"`.
	SetVoiceInputMode { mode: String },
	ToggleVoiceInputMode,
	/// Input volume is 0–100; output volume is 0–200.
	SetInputVolume { value: f32 },
	SetOutputVolume { value: f32 },
	#[serde(rename_all = "camelCase")]
	SetUserVolume { user_id: String, value: f32 },
	#[serde(rename_all = "camelCase")]
	SetInputDevice { device_id: String },
	#[serde(rename_all = "camelCase")]
	SetOutputDevice { device_id: String },
	SetVideo {
		#[serde(skip_serializing_if = "Option::is_none")]
		value: Option<bool>,
	},
	ToggleVideo,
	SetScreenShare {
		#[serde(skip_serializing_if = "Option::is_none")]
		value: Option<bool>,
	},
	ToggleScreenShare,
	/// `null` leaves the current voice channel.
	#[serde(rename_all = "camelCase")]
	SelectVoiceChannel { channel_id: Option<String> },
	/// `guild_id` is optional; the client resolves it from the channel when absent.
	#[serde(rename_all = "camelCase")]
	SelectTextChannel {
		#[serde(skip_serializing_if = "Option::is_none")]
		guild_id: Option<String>,
		channel_id: String,
	},
	#[serde(rename_all = "camelCase")]
	PlaySoundboard { guild_id: String, sound_id: String },
}

/// Events/feedback the client sends to the server.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
	/// Sent immediately on connect.
	Hello {
		#[serde(default)]
		client: String,
		#[serde(default)]
		version: u32,
	},
	/// Full self voice/video state for button feedback.
	#[serde(rename_all = "camelCase")]
	StateUpdate {
		#[serde(default)]
		mute: bool,
		#[serde(default)]
		deaf: bool,
		#[serde(default)]
		input_mode: Option<String>,
		#[serde(default)]
		video: bool,
		#[serde(default)]
		screenshare: bool,
		#[serde(default)]
		channel_id: Option<String>,
	},
	/// Audio device lists/volumes (reply to `requestDevices`).
	Devices(Devices),
	/// Guild + channel list (reply to `requestGuilds`).
	Guilds {
		#[serde(default)]
		guilds: Vec<Guild>,
	},
	/// Soundboard sound list (reply to `requestSoundboard`).
	Soundboard {
		#[serde(default)]
		sounds: Vec<Sound>,
	},
	/// A Discord notification arrived.
	#[serde(rename_all = "camelCase")]
	Notification {
		channel_id: String,
		#[serde(default)]
		title: Option<String>,
		#[serde(default)]
		body: Option<String>,
	},
	/// Forward-compatibility catch-all for unknown `type`s.
	#[serde(other)]
	Unknown,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn server_command_tags_are_camel_case() {
		assert_eq!(
			serde_json::to_value(ServerCommand::Ready).unwrap(),
			serde_json::json!({ "type": "ready" })
		);
		assert_eq!(
			serde_json::to_value(ServerCommand::SetMute { value: true }).unwrap(),
			serde_json::json!({ "type": "setMute", "value": true })
		);
		assert_eq!(
			serde_json::to_value(ServerCommand::SelectVoiceChannel {
				channel_id: Some("123".into())
			})
			.unwrap(),
			serde_json::json!({ "type": "selectVoiceChannel", "channelId": "123" })
		);
		assert_eq!(
			serde_json::to_value(ServerCommand::SetUserVolume {
				user_id: "9".into(),
				value: 50.0
			})
			.unwrap(),
			serde_json::json!({ "type": "setUserVolume", "userId": "9", "value": 50.0 })
		);
		// `guildId` is omitted when None.
		assert_eq!(
			serde_json::to_value(ServerCommand::SelectTextChannel {
				guild_id: None,
				channel_id: "7".into()
			})
			.unwrap(),
			serde_json::json!({ "type": "selectTextChannel", "channelId": "7" })
		);
	}

	#[test]
	fn client_hello_round_trips() {
		let raw = r#"{ "type": "hello", "client": "equibop-opendeck", "version": 1 }"#;
		match serde_json::from_str::<ClientMessage>(raw).unwrap() {
			ClientMessage::Hello { client, version } => {
				assert_eq!(client, "equibop-opendeck");
				assert_eq!(version, 1);
			}
			_ => panic!("expected hello"),
		}
	}

	#[test]
	fn client_state_update_parses() {
		let raw = r#"{ "type": "stateUpdate", "mute": true, "deaf": false,
			"inputMode": "VOICE_ACTIVITY", "video": false, "screenshare": false,
			"channelId": "123456789012345678" }"#;
		match serde_json::from_str::<ClientMessage>(raw).unwrap() {
			ClientMessage::StateUpdate {
				mute,
				deaf,
				input_mode,
				channel_id,
				..
			} => {
				assert!(mute);
				assert!(!deaf);
				assert_eq!(input_mode.as_deref(), Some("VOICE_ACTIVITY"));
				assert_eq!(channel_id.as_deref(), Some("123456789012345678"));
			}
			_ => panic!("expected stateUpdate"),
		}
	}

	#[test]
	fn partial_state_update_uses_defaults() {
		// Missing fields must default rather than fail (forward-compat).
		let raw = r#"{ "type": "stateUpdate", "mute": true }"#;
		assert!(matches!(
			serde_json::from_str::<ClientMessage>(raw).unwrap(),
			ClientMessage::StateUpdate { mute: true, deaf: false, .. }
		));
	}

	#[test]
	fn unknown_message_is_tolerated() {
		let raw = r#"{ "type": "somethingNew", "foo": 1 }"#;
		assert!(matches!(
			serde_json::from_str::<ClientMessage>(raw).unwrap(),
			ClientMessage::Unknown
		));
	}

	#[test]
	fn devices_round_trips() {
		let raw = r#"{ "type": "devices", "input": [{"id":"a","name":"Mic"}],
			"output": [], "currentInput": "a", "currentOutput": "",
			"inputVolume": 80.0, "outputVolume": 150.0 }"#;
		match serde_json::from_str::<ClientMessage>(raw).unwrap() {
			ClientMessage::Devices(d) => {
				assert_eq!(d.input.len(), 1);
				assert_eq!(d.current_input, "a");
				assert_eq!(d.output_volume, 150.0);
			}
			_ => panic!("expected devices"),
		}
	}

	#[test]
	fn guilds_round_trips() {
		let raw = r#"{ "type": "guilds", "guilds": [
			{ "id": "1", "name": "G", "voice": [{"id":"v","name":"V"}], "text": [] } ] }"#;
		match serde_json::from_str::<ClientMessage>(raw).unwrap() {
			ClientMessage::Guilds { guilds } => {
				assert_eq!(guilds.len(), 1);
				assert_eq!(guilds[0].voice[0].id, "v");
			}
			_ => panic!("expected guilds"),
		}
	}
}
