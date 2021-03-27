use json::JsonValue;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc;

use crate::entity::{ChannelProxy, Entity};
use crate::protocol::SmsStatus;

#[derive(Debug)]
pub struct ServerEntity {
	id: u32,
	name: String,
	sp_id: String,
	password: String,
	addr: String,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: u8,
	now_channel_number: u8,
	get_msg_rx: mpsc::Receiver<JsonValue>,
	channels: Vec<ChannelProxy>,
}

impl ServerEntity {
	pub fn new(id: u32,
	           name: String,
	           sp_id: String,
	           password: String,
	           addr: String,
	           read_limit: u32,
	           write_limit: u32,
	           max_channel_number: u8,
	           get_msg_rx: mpsc::Receiver<JsonValue>,
	) -> Self {
		ServerEntity {
			id,
			name,
			sp_id,
			password,
			addr,
			read_limit,
			write_limit,
			max_channel_number,
			now_channel_number: 0,
			get_msg_rx,
			channels: Vec::with_capacity(max_channel_number as usize),
		}
	}

	async fn start(&mut self) {
		unimplemented!()
	}
}

impl Entity for ServerEntity {
	fn login_attach(&self, json: JsonValue) -> (SmsStatus<JsonValue>, u32, u32, Option<Receiver<JsonValue>>, Option<Sender<JsonValue>>) {
		(SmsStatus::OtherError, 0, 0, None, None)
	}

	fn send_message(&self, json: JsonValue) {
		//TODO 收到需要发送的消息的处理。向通道转发消息
		unimplemented!()
	}

	fn get_id(&self) -> u32 {
		let n_id = self.id;
		n_id
	}

	fn get_read_limit(&self) -> u32 {
		self.read_limit
	}

	fn get_write_limit(&self) -> u32 {
		self.write_limit
	}
}

