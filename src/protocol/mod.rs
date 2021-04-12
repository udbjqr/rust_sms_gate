use core::result;
use std::fmt::{Display, Formatter};

use bytes::{BytesMut};
use json::JsonValue;
use tokio::io;

pub use crate::protocol::msg_type::MsgType;
pub use crate::protocol::msg_type::SmsStatus;

pub use self::cmpp48::Cmpp48;
pub use self::sgip::Sgip;
use crate::protocol::names::{MSG_TYPE_U32, MSG_TYPE_STR};
use tokio_util::codec::{Encoder, Decoder};
use crate::protocol::cmpp32::Cmpp32;
use futures::io::Error;
use crate::protocol::smgp::Smgp;
use crate::protocol::Protocol::SMGP;
use crate::protocol::smpp::Smpp;
pub use crate::protocol::implements::ProtocolImpl;

///协议的对应部分。用来编码和解码
mod cmpp48;
mod smgp;
mod sgip;
mod msg_type;
pub mod names;
pub mod cmpp32;
pub mod implements;
mod smpp;

#[derive(Debug, Clone)]
pub enum Protocol {
	CMPP48(Cmpp48),
	CMPP32(Cmpp32),
	SMGP(Smgp),
	SGIP(Sgip),
	SMPP(Smpp),
	None,
}

impl From<&str> for Protocol {
	fn from(name: &str) -> Self {
		//指定某一个协议进行初始化的操作.
		match name[0..4].to_uppercase().as_str() {
			"CMPP" => Protocol::CMPP48(Cmpp48::new()),
			"SMGP" => Protocol::SMGP(Smgp::new()),
			"SGIP" => Protocol::SGIP(Sgip::new()),
			"SMPP" => Protocol::SMPP(Smpp::new()),
			_ => Protocol::None
		}
	}
}


impl Protocol {
	fn parse(&self) -> &str {
		match self {
			Protocol::CMPP48(_) => "CMPP",
			Protocol::CMPP32(_) => "CMPP",
			SMGP(_) => "SMGP",
			Protocol::SGIP(_) => "SGIP",
			Protocol::SMPP(_) => "SMPP",
			Protocol::None => "None"
		}
	}

	///根据给定的版本号.给出相应的处理的对象.
	pub fn match_version(&self, version: u32) -> Self {
		let name = self.parse();

		Protocol::get_protocol(name, version)
	}

	///给定一个版本号。返回与版本号相对应的操作类型
	pub fn get_protocol(name: &str, version: u32) -> Protocol {
		match (name.to_uppercase().as_str(), version) {
			("CMPP", 48) => Protocol::CMPP48(Cmpp48::new()),
			("CMPP", 32) => Protocol::CMPP32(Cmpp32::new()),
			("SGIP", _) => Protocol::SGIP(Sgip::new()),
			("SMGP", _) => Protocol::SMGP(Smgp::new()),
			("SMPP", _) => Protocol::SMPP(Smpp::new()),
			_ => {
				log::error!("未了解的协议名称和版本号.name:{},version:{}", name, version);
				Protocol::CMPP48(Cmpp48::new())
			}
		}
	}

	pub fn has(&self, version: u32) -> bool {
		match self {
			Protocol::CMPP48(_) |
			Protocol::CMPP32(_) => {
				version == 32 || version == 48
			}
			SMGP(_) => true,
			Protocol::SGIP(_) => true,
			Protocol::SMPP(_) => true,
			Protocol::None => false
		}
	}


	pub fn get_auth(&self, login_name: &str, password: &str, timestamp: u32) -> [u8; 16] {
		match self {
			Protocol::CMPP48(obj) => obj.get_auth(login_name, password, timestamp),
			Protocol::CMPP32(obj) => obj.get_auth(login_name, password, timestamp),
			Protocol::SMGP(obj) => obj.get_auth(login_name, password, timestamp),
			Protocol::SGIP(obj) => obj.get_auth(login_name, password, timestamp),
			Protocol::SMPP(obj) => obj.get_auth(login_name, password, timestamp),
			Protocol::None => {
				log::error!("当前操作不可用。");
				[0u8;16]
			}
		}
	}

	pub fn get_status_enum(&self, v: u32) -> SmsStatus {
		match self {
			Protocol::CMPP48(obj) => obj.get_status_enum(v),
			Protocol::CMPP32(obj) => obj.get_status_enum(v),
			Protocol::SMGP(obj) => obj.get_status_enum(v),
			Protocol::SGIP(obj) => obj.get_status_enum(v),
			Protocol::SMPP(obj) => obj.get_status_enum(v),
			Protocol::None => {
				log::error!("当前操作不可用。");
				SmsStatus::UNKNOWN
			}
		}
	}

	///生成实体过来.这里应该是由实体发送来的消息.其他的不在这里。
	pub fn encode_message(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		match self {
			Protocol::CMPP48(obj) => match json[MSG_TYPE_STR].as_str() {
				None => {
					log::error!("解码出错..没有msg_type_str..msg:{}", json);
					Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json)))
				}
				Some(msg_type) => {
					match msg_type.into() {
						MsgType::Connect => obj.encode_connect(json),
						MsgType::Submit => obj.encode_submit(json),
						MsgType::Deliver => obj.encode_deliver(json),
						MsgType::Report => obj.encode_report(json),
						MsgType::ActiveTest => obj.encode_active_test(json),
						_ => {
							log::error!("解码出错..未知的.msg_type..msg:{}", json);
							Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
						}
					}
				}
			}
			Protocol::CMPP32(obj) => match json[MSG_TYPE_STR].as_str() {
				None => {
					log::error!("解码出错..没有msg_type_str..msg:{}", json);
					Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json)))
				}
				Some(msg_type) => {
					match msg_type.into() {
						MsgType::Connect => obj.encode_connect(json),
						MsgType::Submit => obj.encode_submit(json),
						MsgType::Deliver => obj.encode_deliver(json),
						MsgType::Report => obj.encode_report(json),
						MsgType::ActiveTest => obj.encode_active_test(json),
						_ => {
							log::error!("解码出错..未知的.msg_type..msg:{}", json);
							Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
						}
					}
				}
			}
			Protocol::SMGP(obj) => match json[MSG_TYPE_STR].as_str() {
				None => {
					log::error!("解码出错..没有msg_type_str..msg:{}", json);
					Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json)))
				}
				Some(msg_type) => {
					match msg_type.into() {
						MsgType::Connect => obj.encode_connect(json),
						MsgType::Submit => obj.encode_submit(json),
						MsgType::Deliver => obj.encode_deliver(json),
						MsgType::Report => obj.encode_report(json),
						MsgType::ActiveTest => obj.encode_active_test(json),
						_ => {
							log::error!("解码出错..未知的.msg_type..msg:{}", json);
							Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
						}
					}
				}
			}
			Protocol::SGIP(obj) => match json[MSG_TYPE_STR].as_str() {
				None => {
					log::error!("解码出错..没有msg_type_str..msg:{}", json);
					Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json)))
				}
				Some(msg_type) => {
					match msg_type.into() {
						MsgType::Connect => obj.encode_connect(json),
						MsgType::Submit => obj.encode_submit(json),
						MsgType::Deliver => obj.encode_deliver(json),
						MsgType::Report => obj.encode_report(json),
						MsgType::ActiveTest => obj.encode_active_test(json),
						_ => {
							log::error!("解码出错..未知的.msg_type..msg:{}", json);
							Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
						}
					}
				}
			}
			Protocol::SMPP(obj) => match json[MSG_TYPE_STR].as_str() {
				None => {
					log::error!("解码出错..没有msg_type_str..msg:{}", json);
					Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json)))
				}
				Some(msg_type) => {
					match msg_type.into() {
						MsgType::Connect => obj.encode_connect(json),
						MsgType::Submit => obj.encode_submit(json),
						MsgType::Deliver => obj.encode_deliver(json),
						MsgType::Report => obj.encode_report(json),
						MsgType::ActiveTest => obj.encode_active_test(json),
						_ => {
							log::error!("解码出错..未知的.msg_type..msg:{}", json);
							Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
						}
					}
				}
			}
			Protocol::None => {
				log::error!("当前操作不可用。");
				Err(io::Error::new(io::ErrorKind::Other, "还未实现"))
			}
		}
	}

	///为收到的业务消息生成一个回执消息,不包括登录.当本身就是回执消息的时候返回None.
	pub fn encode_receipt(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		match self {
			Protocol::CMPP48(obj) => {
				match obj.get_type_enum(json[MSG_TYPE_U32].as_u32().unwrap()) {
					MsgType::Connect => obj.encode_connect_rep(status, json),
					MsgType::Submit => obj.encode_submit_resp(status, json),
					MsgType::Deliver => obj.encode_deliver_resp(status, json),
					MsgType::Report => obj.encode_report_resp(status, json),
					MsgType::ActiveTest => obj.encode_active_test_resp(status, json),
					_ => None,
				}
			}
			Protocol::CMPP32(obj) => {
				match obj.get_type_enum(json[MSG_TYPE_U32].as_u32().unwrap()) {
					MsgType::Connect => obj.encode_connect_rep(status, json),
					MsgType::Submit => obj.encode_submit_resp(status, json),
					MsgType::Deliver => obj.encode_deliver_resp(status, json),
					MsgType::Report => obj.encode_report_resp(status, json),
					MsgType::ActiveTest => obj.encode_active_test_resp(status, json),
					_ => None,
				}
			}
			Protocol::SMGP(obj) => {
				match obj.get_type_enum(json[MSG_TYPE_U32].as_u32().unwrap()) {
					MsgType::Connect => obj.encode_connect_rep(status, json),
					MsgType::Submit => obj.encode_submit_resp(status, json),
					MsgType::Deliver => obj.encode_deliver_resp(status, json),
					MsgType::Report => obj.encode_report_resp(status, json),
					MsgType::ActiveTest => obj.encode_active_test_resp(status, json),
					_ => None,
				}
			}
			Protocol::SGIP(obj) => {
				match obj.get_type_enum(json[MSG_TYPE_U32].as_u32().unwrap()) {
					MsgType::Connect => obj.encode_connect_rep(status, json),
					MsgType::Submit => obj.encode_submit_resp(status, json),
					MsgType::Deliver => obj.encode_deliver_resp(status, json),
					MsgType::Report => obj.encode_report_resp(status, json),
					MsgType::ActiveTest => obj.encode_active_test_resp(status, json),
					_ => None,
				}
			}
			Protocol::SMPP(obj) => {
				match obj.get_type_enum(json[MSG_TYPE_U32].as_u32().unwrap()) {
					MsgType::Connect => obj.encode_connect_rep(status, json),
					MsgType::Submit => obj.encode_submit_resp(status, json),
					MsgType::Deliver => obj.encode_deliver_resp(status, json),
					MsgType::Report => obj.encode_report_resp(status, json),
					MsgType::ActiveTest => obj.encode_active_test_resp(status, json),
					_ => None,
				}
			}
			Protocol::None => {
				log::error!("当前操作不可用。");
				None
			}
		}
	}
}

impl Display for Protocol {
	fn fmt(&self, f: &mut Formatter<'_>) -> result::Result<(), std::fmt::Error> {
		write!(f, "{:?}", self)
	}
}

impl Encoder<BytesMut> for Protocol {
	type Error = io::Error;

	fn encode(&mut self, item: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
		dst.extend_from_slice(&item);

		log::trace!("发送的字节码..{:X}", dst);
		Ok(())
	}
}

impl Decoder for Protocol {
	type Item = JsonValue;
	type Error = io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		match self {
			Protocol::CMPP48(obj) => obj.decode_read_msg(src),
			Protocol::CMPP32(obj) => obj.decode_read_msg(src),
			Protocol::SMGP(obj) => obj.decode_read_msg(src),
			Protocol::SGIP(obj) => obj.decode_read_msg(src),
			Protocol::SMPP(obj) => obj.decode_read_msg(src),
			Protocol::None => {
				log::error!("当前操作不可用。");
				Ok(None)
			}
		}
	}
}
