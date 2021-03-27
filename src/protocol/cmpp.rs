use bytes::{Buf, BufMut, BytesMut};
use chrono::{Datelike, DateTime, Local, Timelike};
use json::JsonValue;
use log::info;
use tokio::io;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::protocol::{MsgType, Protocol};
use crate::protocol::msg_type::MsgType::Connect;
use tokio::io::Error;
use crate::global::get_sequence_id;
use crate::protocol::msg_type::SmsStatus;

///CMPP协议3.0的处理
#[derive(Debug, Default)]
pub struct Cmpp48 {
	codec: LengthDelimitedCodec,
}

impl Protocol for Cmpp48 {
	fn e_receipt(&self, json: &JsonValue) -> Option<BytesMut> {
		match self.get_type_enum(json["t"].as_u32().unwrap()) {
			MsgType::Submit => Some(self.e_submit_resp(json)),
			MsgType::Deliver => Some(self.e_deliver_resp(json)),
			_ => None
		}
	}

	fn e_submit_resp(&self, json: &JsonValue) -> BytesMut {
		unimplemented!()
	}

	fn e_deliver_resp(&self, json: &JsonValue) -> BytesMut {
		unimplemented!()
	}

	fn e_message(&self, json: &JsonValue) -> Result<BytesMut, Error> {
		unimplemented!()
	}


	fn e_login_msg(&self, sp_id: &str, password: &str, version: u8) -> Result<BytesMut, io::Error> {
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
		dst.put_u8(version);
		dst.put_u32(time);

		Ok(dst)
	}

	fn e_login_rep_msg(&self, _json: &SmsStatus<JsonValue>) -> BytesMut {
		unimplemented!()
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

	fn get_type_id(&self,t: MsgType) -> u32 {
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

	fn get_type_enum(&self,v: u32) -> MsgType {
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

	fn get_status_id(&self, t: SmsStatus<JsonValue>) -> u32 {
		unimplemented!()
	}

	fn get_status_enum(&self, v: u32) -> SmsStatus<JsonValue> {
		unimplemented!()
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

	fn decode_connect_resp(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json["Sequence_Id"] = seq.into();
		json["Status"] = buf.get_u32().into();
		buf.advance(16);
		json["Version"] = buf.get_u8().into();

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, io::Error> {
		//TODO 解码的过程
		info!("现在还未实现.{:?},seq:{}", buf, seq);
		Ok(JsonValue::new_object())
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
		match self.codec.decode(src){
			Ok(Some(mut src)) => {
				self.decode_read_msg(&mut src)
			}
			Ok(None) => Ok(None),
			Err(e) => Err(e)
		}

	}
}
