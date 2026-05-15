use tokio::process::Command;

pub async fn run_equibop(arg: &'static str) {
	match Command::new("equibop").arg(arg).spawn() {
		Ok(_) => {}
		Err(e) => log::error!("Failed to spawn `equibop {}`: {}", arg, e),
	}
}
