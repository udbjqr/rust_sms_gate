use crate::global::FILL_ZERO;
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
pub mod names;

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
	fn encode_receipt(&self, json: &JsonValue) -> Option<BytesMut>;
	fn encode_submit_resp(&self, json: &JsonValue) -> BytesMut;
	fn encode_deliver_resp(&self, json: &JsonValue) -> BytesMut;
	///生成实体过来.这里应该是只会有单独的几个消息.其他的不应该在这里。
	fn encode_from_entity_message(&self, json: &JsonValue) -> Result<BytesMut, io::Error>;

	fn encode_login_msg(&self, user_name: &str, password: &str, version: &str) -> Result<BytesMut, io::Error>;
	///根据对方给的请求,处理以后的编码消息
	fn encode_login_rep(&self, status: &SmsStatus<JsonValue>) -> BytesMut;

	fn decode_read_msg(&self, buf: &mut BytesMut) -> io::Result<Option<JsonValue>>;

	fn get_type_id(&self, t: MsgType) -> u32;
	fn get_type_enum(&self, v: u32) -> MsgType;
	///通过给出的状态值.返回对应发给协议对端的u32值.
	fn get_status_id<T>(&self, status: &SmsStatus<T>) -> u32;
	fn get_status_enum<T>(&self, v: u32, json: T) -> SmsStatus<T>;
}

///向缓冲写入指定长度数据.如果数据不够长.使用0进行填充.
pub fn fill_bytes_zero(dest: &mut BytesMut, slice: &str, len: usize) {
	if slice.len() >= len {
		dest.extend_from_slice(slice[0..len].as_ref());
	} else {
		dest.extend_from_slice(slice.as_ref());
		dest.extend_from_slice(&FILL_ZERO[0..len - slice.len()]);
	}
}
