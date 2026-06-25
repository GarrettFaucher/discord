//! The localhost WebSocket **server** half of the bridge. The Equibop userplugin connects
//! as a client; action handlers reach it through [`send_command`]. Only one client is tracked
//! at a time (last connection wins).

use crate::feedback;
use crate::protocol::{ClientMessage, ServerCommand};

use std::net::SocketAddr;
use std::sync::OnceLock;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::tungstenite::Message;

type ClientTx = mpsc::UnboundedSender<Message>;

/// The sender for the currently connected client, if any.
fn client_tx() -> &'static RwLock<Option<ClientTx>> {
	static TX: OnceLock<RwLock<Option<ClientTx>>> = OnceLock::new();
	TX.get_or_init(|| RwLock::new(None))
}

/// Serialize and deliver a command to the connected client.
///
/// Returns `Err(())` when no client is connected (or the channel is closed), so callers can
/// `show_alert()`.
pub async fn send_command(command: ServerCommand) -> Result<(), ()> {
	let guard = client_tx().read().await;
	let Some(tx) = guard.as_ref() else {
		log::warn!("No Equibop client connected; dropping {command:?}");
		return Err(());
	};

	let json = serde_json::to_string(&command).map_err(|e| {
		log::error!("Failed to serialize command: {e}");
	})?;

	tx.send(Message::text(json)).map_err(|e| {
		log::error!("Failed to send command to client: {e}");
	})
}

/// Bind and run the bridge server until the task is aborted.
pub async fn serve(port: u16) {
	let addr = format!("127.0.0.1:{port}");
	let listener = match TcpListener::bind(&addr).await {
		Ok(listener) => listener,
		Err(e) => {
			log::error!("Failed to bind Equibop bridge on {addr}: {e}");
			return;
		}
	};

	log::info!("Equibop bridge listening on {addr}");

	loop {
		match listener.accept().await {
			Ok((stream, peer)) => {
				tokio::spawn(handle_connection(stream, peer));
			}
			Err(e) => log::error!("Failed to accept connection: {e}"),
		}
	}
}

async fn handle_connection(stream: TcpStream, peer: SocketAddr) {
	let ws = match tokio_tungstenite::accept_async(stream).await {
		Ok(ws) => ws,
		Err(e) => {
			log::error!("WebSocket handshake with {peer} failed: {e}");
			return;
		}
	};

	log::info!("Equibop client connected from {peer}");

	let (mut sink, mut stream) = ws.split();
	let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

	// Register as the active client (last connection wins).
	*client_tx().write().await = Some(tx.clone());

	// Pump outbound commands from the channel to the socket.
	let writer = tokio::spawn(async move {
		while let Some(msg) = rx.recv().await {
			if sink.send(msg).await.is_err() {
				break;
			}
		}
	});

	while let Some(msg) = stream.next().await {
		match msg {
			Ok(Message::Text(text)) => handle_text(text.as_str()).await,
			Ok(Message::Close(_)) => break,
			Ok(_) => {}
			Err(e) => {
				log::error!("WebSocket error from {peer}: {e}");
				break;
			}
		}
	}

	// Only clear the active client if it is still us (a newer client may have replaced it).
	{
		let mut guard = client_tx().write().await;
		if guard.as_ref().is_some_and(|active| active.same_channel(&tx)) {
			*guard = None;
			feedback::on_client_disconnected().await;
		}
	}

	writer.abort();
	log::info!("Equibop client {peer} disconnected");
}

async fn handle_text(text: &str) {
	let message: ClientMessage = match serde_json::from_str(text) {
		Ok(message) => message,
		Err(e) => {
			log::warn!("Ignoring unparseable client message: {e}");
			return;
		}
	};

	match message {
		ClientMessage::Hello { client, version } => {
			log::info!("Equibop client hello: {client} (protocol v{version})");
			// Handshake, then prime every cache from the freshly connected client.
			let _ = send_command(ServerCommand::Ready).await;
			let _ = send_command(ServerCommand::RequestState).await;
			let _ = send_command(ServerCommand::RequestGuilds).await;
			let _ = send_command(ServerCommand::RequestSoundboard).await;
			let _ = send_command(ServerCommand::RequestDevices).await;
		}
		ClientMessage::StateUpdate {
			mute,
			deaf,
			input_mode,
			video,
			screenshare,
			channel_id,
		} => {
			feedback::apply_state_update(mute, deaf, input_mode, video, screenshare, channel_id)
				.await;
		}
		ClientMessage::Devices(devices) => feedback::apply_devices(devices).await,
		ClientMessage::Guilds { guilds } => feedback::apply_guilds(guilds).await,
		ClientMessage::Soundboard { sounds } => feedback::apply_soundboard(sounds).await,
		ClientMessage::Notification {
			channel_id,
			title,
			body,
		} => feedback::apply_notification(channel_id, title, body).await,
		ClientMessage::Unknown => {}
	}
}
