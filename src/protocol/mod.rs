use crate::global::{FILL_ZERO, ISMG_ID, get_sequence_id};
use core::result;
use std::fmt::{Display, Formatter};

use bytes::{BytesMut, Buf, Bytes};
use json::JsonValue;
use tokio::io;

pub use crate::protocol::msg_type::MsgType;
pub use crate::protocol::msg_type::SmsStatus;

pub use self::cmpp::Cmpp48;
pub use self::sgip::Sgip;
use chrono::{Local, Datelike, Timelike};
use crate::protocol::names::{LONG_SMS_TOTAL, LONG_SMS_NOW_NUMBER, MSG_CONTENT};
use encoding::all::UTF_16BE;
use encoding::{Encoding, DecoderTrap};

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
	///为收到的消息生成一个回执消息.当本身就是回执消息的时候返回None.
	fn encode_receipt(&self, status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut>;
	///生成实体过来.这里应该是只会有单独的几个消息.其他的不应该在这里。
	fn encode_from_entity_message(&self, json: &JsonValue) -> Result<(BytesMut, Vec<u32>), io::Error>;

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

///读一个长度的utf-8 字符编码.确保是utf-8.并且当前值是有效值.
pub fn load_utf8_string(buf: &mut BytesMut, len: usize) -> String {
	unsafe {
		String::from_utf8_unchecked(Vec::from(copy_to_bytes(buf, len).as_ref())).trim_end_matches(char::from(0)).to_owned()
	}
}

///读一个长度字串.做一个异常的保护.
pub fn copy_to_bytes(buf: &mut BytesMut, len: usize) -> Bytes {
	if buf.len() > len {
		buf.split_to(len).freeze()
	} else {
		log::error!("得到消息出现错误.消息没有足够长度.可用长度:{}.现有长度{}", buf.len(), len);
		Bytes::new()
	}
}

///返回一个msg_id.目前在Cmpp协议里面使用
pub fn get_msg_id() -> u64 {
	let date = Local::now();
	let mut result: u64 = date.month() as u64;
	result = result << 5 | (date.day() & 0x1F) as u64;
	result = result << 5 | (date.hour() & 0x1F) as u64;
	result = result << 6 | (date.minute() & 0x3F) as u64;
	result = result << 6 | (date.second() & 0x3F) as u64;
	result = result << 22 | *ISMG_ID as u64;

	result << 16 | (get_sequence_id(1) & 0xff) as u64
}

///处理短信内容的通用方法.会处理msg_length和msg_content.需要这两个是连续的.
pub fn decode_msg_content(buf: &mut BytesMut, msg_fmt: u8, json: &mut JsonValue, is_long_sms: bool) -> Result<(), io::Error> {
	let mut msg_content_len = buf.get_u8(); //Msg_Length	1

	if is_long_sms {
		let head_len = buf.get_u8(); //接下来的长度
		buf.advance((head_len - 2) as usize); //跳过对应的长度
		json[LONG_SMS_TOTAL] = buf.get_u8().into();
		json[LONG_SMS_NOW_NUMBER] = buf.get_u8().into();

		if msg_content_len > (head_len + 1) {
			msg_content_len = msg_content_len - head_len - 1;
		} else {
			log::warn!("消息结构出错.整体消息长度小于消息头长度.");
			return Err(io::Error::new(io::ErrorKind::Other, "消息结构出错."));
		}
	}

	let msg_content = copy_to_bytes(buf, msg_content_len as usize);
	//根据字符集转换.
	let msg_content = match msg_fmt {
		8 => match UTF_16BE.decode(&msg_content, DecoderTrap::Strict) {
			Ok(v) => v,
			Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e))
		}
		0 => match std::str::from_utf8(&msg_content) {
			Ok(v) => v.to_owned(),
			Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e))
		},
		_ => {
			log::warn!("未处理的字符集类型.跳过.msg_fmt:{}", msg_fmt);
			return Err(io::Error::new(io::ErrorKind::Other, "未处理的字符集类型.跳过"));
		}
	};

	json[MSG_CONTENT] = msg_content.into();

	Ok(())
}
