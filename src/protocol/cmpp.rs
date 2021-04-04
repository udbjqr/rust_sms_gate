use bytes::{Buf, BufMut, BytesMut};
use chrono::{Datelike, DateTime, Local, Timelike};
use json::JsonValue;
use tokio::io;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::protocol::{MsgType, Protocol, fill_bytes_zero};
use crate::protocol::msg_type::MsgType::Connect;
use tokio::io::Error;
use crate::global::{get_sequence_id, FILL_ZERO};
use crate::protocol::msg_type::SmsStatus;
use crate::protocol::names::{SEQ_ID, AUTHENTICATOR, VERSION, STATUS, MSG_TYPE, USER_ID, MSG_CONTENT, MSG_ID, SERVICE_ID, TP_UDHI, SP_ID, VALID_TIME, AT_TIME, SRC_ID, DEST_ID};
use encoding::all::UTF_16BE;
use encoding::{Encoding, EncoderTrap};

///CMPP协议3.0的处理
#[derive(Debug, Default)]
pub struct Cmpp48 {
	codec: LengthDelimitedCodec,
}

impl Protocol for Cmpp48 {
	fn encode_receipt(&self, json: &JsonValue) -> Option<BytesMut> {
		match self.get_type_enum(json["t"].as_u32().unwrap()) {
			MsgType::Submit => Some(self.encode_submit_resp(json)),
			MsgType::Deliver => Some(self.encode_deliver_resp(json)),
			_ => None
		}
	}

	fn encode_submit_resp(&self, _json: &JsonValue) -> BytesMut {
		unimplemented!()
	}

	fn encode_deliver_resp(&self, _json: &JsonValue) -> BytesMut {
		unimplemented!()
	}

	fn encode_from_entity_message(&self, json: &JsonValue) -> Result<BytesMut, Error> {
		match json[MSG_TYPE].as_u32() {
			None => Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json))),
			Some(msg_type) => {
				match self.get_type_enum(msg_type) {
					//TODO : 完成这个部分.
					MsgType::Submit => self.encode_submit(json),
					MsgType::Deliver |
					MsgType::Report |
					_ => Err(io::Error::new(io::ErrorKind::Other, "还未实现")),
				}
			}
		}
	}


	fn encode_login_msg(&self, sp_id: &str, password: &str, version: &str) -> Result<BytesMut, io::Error> {
		if sp_id.len() != 6 {
			return Err(io::Error::new(io::ErrorKind::Other, "sp_id,长度应该为6位"));
		}

		let local: DateTime<Local> = Local::now();
		let mut time: u32 = local.month();
		time = time * 100 + local.day();
		time = time * 100 + local.hour();
		time = time * 100 + local.minute();
		time = time * 100 + local.second();

		let time_str = format!("{:010}", time);

		//用于鉴别源地址。其值通过单向MD5 hash计算得出，表示如下：
		// AuthenticatorSource =
		// MD5（Source_Addr+9 字节的0 +shared secret+timestamp）
		// Shared secret 由中国移动与源地址实体事先商定，timestamp格式为：MMDDHHMMSS，即月日时分秒，10位。
		let mut src = BytesMut::with_capacity(sp_id.len() + 9 + password.len() + 10);

		src.put_slice(sp_id.as_bytes());
		//加起来9位
		src.put_u64(0);
		src.put_u8(0);

		src.put_slice(password.as_bytes());
		src.put_slice(time_str.as_bytes());

		let (src, _) = src.split_at(src.len());
		let auth = md5::compute(src);

		//固定这个大小。
		let mut dst = BytesMut::with_capacity(39);
		dst.put_u32(39);
		dst.put_u32(self.get_type_id(Connect));
		dst.put_u32(get_sequence_id(1));
		dst.extend_from_slice(sp_id.as_bytes());
		dst.extend_from_slice(auth.as_ref());
		match version {
			"2.0" => dst.put_u8(32),
			"3.0" => dst.put_u8(48),
			_ => dst.put_u8(48)
		}
		dst.put_u32(time);

		Ok(dst)
	}

	fn encode_login_rep(&self, status: &SmsStatus<JsonValue>) -> BytesMut {
		match status {
			SmsStatus::Success(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::AddError(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::AuthError(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::VersionError(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::LoginOtherError(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::TrafficRestrictions(json) => self.login_rep(self.get_status_id(status), json),
			SmsStatus::MessageError(json) => self.login_rep(self.get_status_id(status), json),
		}
	}

	///处理送过来的消息。因为之前使用的解码器是在单线程内。
	/// 所以只处理断包和粘包。
	/// 这里才是真正处理数据的地方。
	/// 到这里应该已经处理掉了长度的头。
	fn decode_read_msg(&self, buf: &mut BytesMut) -> io::Result<Option<JsonValue>> {
		//拿command
		let tp = buf.get_u32();
		//拿掉seq
		let seq = buf.get_u32();

		let msg = match self.get_type_enum(tp) {
			MsgType::Submit |
			MsgType::SubmitResp |
			MsgType::Deliver |
			MsgType::DeliverResp |
			MsgType::ActiveTest |
			MsgType::ActiveTestResp |
			MsgType::Connect => self.decode_connect(buf, seq)?,
			MsgType::ConnectResp => self.decode_connect_resp(buf, seq)?,
			MsgType::Terminate |
			MsgType::TerminateResp |
			MsgType::Query |
			MsgType::QueryResp |
			MsgType::Cancel |
			MsgType::CancelResp |
			MsgType::Fwd |
			MsgType::FwdResp |
			MsgType::MtRoute |
			MsgType::MtRouteResp |
			MsgType::MoRoute |
			MsgType::MoRouteResp |
			MsgType::GetMtRoute |
			MsgType::GetMtRouteResp |
			MsgType::MtRouteUpdate |
			MsgType::MtRouteUpdateResp |
			MsgType::MoRouteUpdate |
			MsgType::MoRouteUpdateResp |
			MsgType::PushMtRouteUpdate |
			MsgType::PushMtRouteUpdateResp |
			MsgType::PushMoRouteUpdate |
			MsgType::PushMoRouteUpdateResp |
			MsgType::GetMoRoute |
			MsgType::ReportResp |
			MsgType::UnKnow |
			MsgType::Report |
			MsgType::GetMoRouteResp =>
				return Err(io::Error::new(io::ErrorKind::Other, "还未实现")),
			// _ => return Err(io::Error::new(io::ErrorKind::NotFound, "type没值,或者无法转换")),
		};

		Ok(Some(msg))
	}

	fn get_type_id(&self, t: MsgType) -> u32 {
		match t {
			MsgType::Submit => 0x00000004,
			MsgType::SubmitResp => 0x80000004,
			MsgType::Deliver => 0x00000005,
			MsgType::DeliverResp => 0x80000005,
			MsgType::Report => 0x00000005,
			MsgType::ReportResp => 0x80000005,
			MsgType::Connect => 0x00000001,
			MsgType::ConnectResp => 0x80000001,
			MsgType::Terminate => 0x00000002,
			MsgType::TerminateResp => 0x80000002,
			MsgType::Query => 0x00000006,
			MsgType::QueryResp => 0x80000006,
			MsgType::Cancel => 0x00000007,
			MsgType::CancelResp => 0x80000007,
			MsgType::ActiveTest => 0x00000008,
			MsgType::ActiveTestResp => 0x80000008,
			MsgType::Fwd => 0x00000009,
			MsgType::FwdResp => 0x80000009,
			MsgType::MtRoute => 0x00000010,
			MsgType::MtRouteResp => 0x80000010,
			MsgType::MoRoute => 0x00000011,
			MsgType::MoRouteResp => 0x80000011,
			MsgType::GetMtRoute => 0x00000012,
			MsgType::GetMtRouteResp => 0x80000012,
			MsgType::MtRouteUpdate => 0x00000013,
			MsgType::MtRouteUpdateResp => 0x80000013,
			MsgType::MoRouteUpdate => 0x00000014,
			MsgType::MoRouteUpdateResp => 0x80000014,
			MsgType::PushMtRouteUpdate => 0x00000015,
			MsgType::PushMtRouteUpdateResp => 0x80000015,
			MsgType::PushMoRouteUpdate => 0x00000016,
			MsgType::PushMoRouteUpdateResp => 0x80000016,
			MsgType::GetMoRoute => 0x00000017,
			MsgType::GetMoRouteResp => 0x80000017,
			MsgType::UnKnow => 0,
		}
	}

	fn get_type_enum(&self, v: u32) -> MsgType {
		match v {
			0x00000004 => MsgType::Submit,
			0x80000004 => MsgType::SubmitResp,
			0x00000005 => MsgType::Deliver,
			0x80000005 => MsgType::DeliverResp,
			0x00000001 => MsgType::Connect,
			0x80000001 => MsgType::ConnectResp,
			0x00000002 => MsgType::Terminate,
			0x80000002 => MsgType::TerminateResp,
			0x00000006 => MsgType::Query,
			0x80000006 => MsgType::QueryResp,
			0x00000007 => MsgType::Cancel,
			0x80000007 => MsgType::CancelResp,
			0x00000008 => MsgType::ActiveTest,
			0x80000008 => MsgType::ActiveTestResp,
			0x00000009 => MsgType::Fwd,
			0x80000009 => MsgType::FwdResp,
			0x00000010 => MsgType::MtRoute,
			0x80000010 => MsgType::MtRouteResp,
			0x00000011 => MsgType::MoRoute,
			0x80000011 => MsgType::MoRouteResp,
			0x00000012 => MsgType::GetMtRoute,
			0x80000012 => MsgType::GetMtRouteResp,
			0x00000013 => MsgType::MtRouteUpdate,
			0x80000013 => MsgType::MtRouteUpdateResp,
			0x00000014 => MsgType::MoRouteUpdate,
			0x80000014 => MsgType::MoRouteUpdateResp,
			0x00000015 => MsgType::PushMtRouteUpdate,
			0x80000015 => MsgType::PushMtRouteUpdateResp,
			0x00000016 => MsgType::PushMoRouteUpdate,
			0x80000016 => MsgType::PushMoRouteUpdateResp,
			0x00000017 => MsgType::GetMoRoute,
			0x80000017 => MsgType::GetMoRouteResp,
			_ => MsgType::UnKnow,
		}
	}

	fn get_status_id<T>(&self, status: &SmsStatus<T>) -> u32 {
		match status {
			SmsStatus::Success(_) => 0,
			SmsStatus::MessageError(_) => 1,
			SmsStatus::AddError(_) => 2,
			SmsStatus::AuthError(_) => 3,
			SmsStatus::VersionError(_) => 4,
			SmsStatus::TrafficRestrictions(_) => 8,
			SmsStatus::LoginOtherError(_) => 5
		}
	}

	fn get_status_enum<T>(&self, v: u32, json: T) -> SmsStatus<T> {
		match v {
			0 => SmsStatus::Success(json),
			1 => SmsStatus::MessageError(json),
			2 => SmsStatus::AddError(json),
			3 => SmsStatus::AuthError(json),
			4 => SmsStatus::VersionError(json),
			8 => SmsStatus::TrafficRestrictions(json),
			5 => SmsStatus::LoginOtherError(json),
			_ => { SmsStatus::LoginOtherError(json) }
		}
	}
}

impl Clone for Cmpp48 {
	fn clone(&self) -> Self {
		Cmpp48::new()
	}
}

impl Cmpp48 {
	pub fn new() -> Self {
		Cmpp48 {
			codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}

	///生成待发送消息.这里应该已经是拆开了长短信的.
	fn encode_submit(&self, json: &JsonValue) -> Result<BytesMut, Error> {
		let msg_content = match json[MSG_CONTENT].as_str() {
			None => {
				log::error!("没有内容字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有内容字串"));
			}
			Some(v) => v
		};

		let msg_content = match UTF_16BE.encode(msg_content, EncoderTrap::Strict) {
			Ok(v) => v,
			Err(e) => {
				log::error!("字符串内容解码出现错误..json:{}.e:{}", json, e);
				return Err(io::Error::new(io::ErrorKind::Other, "字符串内容解码出现错误"));
			}
		};

		let msg_id = match json[MSG_ID].as_u64() {
			Some(v) => v,
			None => {
				log::error!("没有msg_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id"));
			}
		};

		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有msg_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id"));
			}
		};

		let tp_udhi = match json[TP_UDHI].as_u8() {
			Some(v) => v,
			None => {
				log::error!("没有tp_udhi.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有tp_udhi"));
			}
		};

		let sp_id = match json[SP_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有sp_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有sp_id"));
			}
		};

		let valid_time = match json[VALID_TIME].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有valid_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有valid_time"));
			}
		};

		let at_time = match json[AT_TIME].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有at_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有at_time"));
			}
		};

		let src_id = match json[SRC_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有src_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有src_id"));
			}
		};

		let dest_id = match json[DEST_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有dest_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有dest_id"));
			}
		};


		let msg_content_len = msg_content.len();

		//195是除开内容之后所有长度加在一起
		let mut dst = BytesMut::with_capacity(195 + msg_content_len);

		dst.put_u32((195 + msg_content_len) as u32);
		dst.put_u32(self.get_type_id(MsgType::Submit));
		dst.put_u32(get_sequence_id(1));

		dst.put_u64(msg_id); //Msg_Id
		dst.put_u8(1); //Pk_total
		dst.put_u8(1); //Pk_number
		dst.put_u8(1); //Registered_Delivery
		dst.put_u8(1); //Msg_level
		fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
		dst.put_u8(3); //Fee_UserType
		dst.extend_from_slice(&FILL_ZERO[0..32]);//Fee_terminal_Id
		dst.put_u8(0); //Fee_terminal_type
		dst.put_u8(0); //TP_pId
		dst.put_u8(tp_udhi); //TP_pId
		dst.put_u8(8); //Msg_Fmt
		dst.extend_from_slice(sp_id[0..6].as_ref()); //sp_id
		dst.extend_from_slice("01".as_ref()); //FeeType
		dst.extend_from_slice("000001".as_ref()); //FeeCode
		fill_bytes_zero(&mut dst, valid_time, 17);  //valid_time
		fill_bytes_zero(&mut dst, at_time, 17);  //at_time
		fill_bytes_zero(&mut dst, src_id, 21);  //src_id
		dst.put_u8(1); //DestUsr_tl
		fill_bytes_zero(&mut dst, dest_id, 32);  //dest_id
		dst.put_u8(0); //Dest_terminal_type
		dst.put_u8(msg_content_len as u8); //Msg_Length
		dst.extend_from_slice(&msg_content[..]); //Msg_Content
		dst.extend_from_slice(&FILL_ZERO[0..20]); //LinkID

		Ok(dst)
	}

	fn login_rep(&self, status: u32, json: &JsonValue) -> BytesMut {
		let mut dst = BytesMut::with_capacity(33);
		dst.put_u32(33);
		dst.put_u32(self.get_type_id(MsgType::ConnectResp));

		dst.put_u32(json[SEQ_ID].as_u32().unwrap());
		dst.put_u32(status);

		let str = json[AUTHENTICATOR].as_str().unwrap().as_bytes();

		dst.extend_from_slice(&str[0..16]);
		dst.put_u8(json[VERSION].as_u8().unwrap());

		dst
	}

	fn decode_connect_resp(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = (0x80000001 as u32).into();
		json[SEQ_ID] = seq.into();
		json[STATUS] = buf.get_u32().into();
		buf.advance(16);
		json[VERSION] = buf.get_u8().into();

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = (0x00000001 as u32).into();
		json[SEQ_ID] = seq.into();
		let s: String = unsafe {
			String::from_utf8_unchecked(Vec::from(buf.copy_to_bytes(6).as_ref()))
		};
		json[USER_ID] = s.into();
		json[AUTHENTICATOR] = buf.copy_to_bytes(16).as_ref().into();
		json[VERSION] = buf.get_u8().into();

		Ok(json)
	}
}

impl Encoder<BytesMut> for Cmpp48 {
	type Error = io::Error;

	fn encode(&mut self, item: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
		dst.extend_from_slice(&*item);

		Ok(())
	}
}

impl Decoder for Cmpp48 {
	type Item = JsonValue;
	type Error = io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		match self.codec.decode(src) {
			Ok(Some(mut src)) => {
				self.decode_read_msg(&mut src)
			}
			Ok(None) => Ok(None),
			Err(e) => Err(e)
		}
	}
}
