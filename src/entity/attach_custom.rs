use json::JsonValue;
use tokio::sync::mpsc;

use crate::entity::{Entity};
use crate::protocol::SmsStatus;

#[derive(Debug)]
pub struct CustomEntity {
	id: u32,
	name: String,
	sp_id: String,
	password: String,
	allowed_addr: Vec<String>,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: u8,
	now_channel_number: u8,
}

impl CustomEntity {
	pub fn new(id: u32,
	           name: String,
	           sp_id: String,
	           password: String,
	           allowed_addr: Vec<String>,
	           read_limit: u32,
	           write_limit: u32,
	           max_channel_number: u8,
	) -> Self {
		CustomEntity {
			id,
			name,
			sp_id,
			password,
			allowed_addr,
			read_limit,
			write_limit,
			max_channel_number,
			now_channel_number: 0,
		}
	}

	async fn start(&mut self) {
		unimplemented!()
	}
}

impl Entity for CustomEntity {
	fn login_attach(&self, json: JsonValue) -> (SmsStatus<JsonValue>, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>) {
		if self.now_channel_number >= self.max_channel_number {
			return (SmsStatus::OtherError, 0, 0, None, None);
		}

		//TODO 进行地址允许判断
		//TODO 进行登录判断。
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
		unimplemented!()
	}

	fn get_write_limit(&self) -> u32 {
		unimplemented!()
	}
}

