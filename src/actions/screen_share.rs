use crate::protocol::ServerCommand;
use crate::ws_server::send_command;

use std::collections::HashMap;

use openaction::{Action, ActionUuid, Instance, OpenActionResult, async_trait};

pub struct ToggleScreenshareAction;

#[async_trait]
impl Action for ToggleScreenshareAction {
	const UUID: ActionUuid = "me.amankhanna.oadiscord.togglescreenshare";
	type Settings = HashMap<String, String>;

	async fn key_up(
		&self,
		instance: &Instance,
		_settings: &Self::Settings,
	) -> OpenActionResult<()> {
		// State feedback arrives via the client's `stateUpdate`, so no optimistic state here.
		if send_command(ServerCommand::ToggleScreenShare).await.is_err() {
			instance.show_alert().await?;
		}

		Ok(())
	}
}
