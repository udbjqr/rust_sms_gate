use json::JsonValue;
use tokio::sync::mpsc;

mod channel;
mod server_entity;
mod client_entity;
mod services;


pub trait Entity :Send + Sync {
	fn get_msg_send_tx(&self) -> mpsc::Sender<JsonValue>;
}
