use core::result;
use std::fmt::{Display, Formatter};

use bytes::BytesMut;
use json::JsonValue;
use tokio::io;

pub use crate::protocol::msg_type::MsgType;
pub use crate::protocol::msg_type::SmsStatus;

pub use self::cmpp::Cmpp48;
pub use self::sgip::Sgip;

///协议的对应部分。用来编码和解码

mod cmpp;
mod smgp;
mod sgip;
mod msg_type;

#[derive(Debug, Copy, Clone)]
pub enum ProtocolType {
	CMPP,
	SMGP,
	SGIP,
	SMPP,
	UNKNOWN,
}

impl From<&str> for ProtocolType {
	fn from(id: &str) -> Self {
		match id {
			"CMPP" => ProtocolType::CMPP,
			"SMGP" => ProtocolType::SMGP,
			"SGIP" => ProtocolType::SGIP,
			"SMPP" => ProtocolType::SMPP,
			_ => ProtocolType::UNKNOWN
		}
	}
}

impl Display for ProtocolType {
	fn fmt(&self, f: &mut Formatter<'_>) -> result::Result<(), std::fmt::Error> {
		write!(f, "{:?}", self)
	}
}

pub trait Protocol: Send + Sync {
	fn e_receipt(&self, json: &JsonValue) -> Option<BytesMut>;
	fn e_submit_resp(&self, json: &JsonValue) -> BytesMut;
	fn e_deliver_resp(&self, json: &JsonValue) -> BytesMut;
	///生成消息的操作。
	fn e_message(&self, json: &JsonValue) -> Result<BytesMut, io::Error>;

	fn e_login_msg(&self, user_name: &str, password: &str, version: &str) -> Result<BytesMut, io::Error>;
	///根据对方给的请求,处理以后的编码消息
	fn e_login_rep_msg(&self, status: &SmsStatus<JsonValue>) -> BytesMut;

	fn decode_read_msg(&self, buf: &mut BytesMut) -> io::Result<Option<JsonValue>>;

	fn get_type_id(&self, t: MsgType) -> u32;
	fn get_type_enum(&self, v: u32) -> MsgType;
	fn get_status_id(&self, t: SmsStatus<JsonValue>) -> u32;
	fn get_status_enum(&self, v: u32) -> SmsStatus<JsonValue>;
}

