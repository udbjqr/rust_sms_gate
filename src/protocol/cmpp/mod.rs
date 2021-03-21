use std::io::{self};
use std::io::Error;

use bytes::{Buf, BufMut, BytesMut};
use chrono::{Datelike, DateTime, Local, Timelike};
use json::JsonValue;
use log::info;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::protocol::Protocol;

#[derive(Debug)]
pub struct Cmpp {
	codec: LengthDelimitedCodec,
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum CmppType {
	CmppConnect = 0x00000001,
	CmppConnectResp = 0x80000001,
	CmppTerminate = 0x00000002,
	CmppTerminateResp = 0x80000002,
	CmppSubmit = 0x00000004,
	CmppSubmitResp = 0x80000004,
	CmppDeliver = 0x00000005,
	CmppDeliverResp = 0x80000005,
	CmppQuery = 0x00000006,
	CmppQueryResp = 0x80000006,
	CmppCancel = 0x00000007,
	CmppCancelResp = 0x80000007,
	CmppActiveTest = 0x00000008,
	CmppActiveTestResp = 0x80000008,
	CmppFwd = 0x00000009,
	CmppFwdResp = 0x80000009,
	CmppMtRoute = 0x00000010,
	CmppMtRouteResp = 0x80000010,
	CmppMoRoute = 0x00000011,
	CmppMoRouteResp = 0x80000011,
	CmppGetMtRoute = 0x00000012,
	CmppGetMtRouteResp = 0x80000012,
	CmppMtRouteUpdate = 0x00000013,
	CmppMtRouteUpdateResp = 0x80000013,
	CmppMoRouteUpdate = 0x00000014,
	CmppMoRouteUpdateResp = 0x80000014,
	CmppPushMtRouteUpdate = 0x00000015,
	CmppPushMtRouteUpdateResp = 0x80000015,
	CmppPushMoRouteUpdate = 0x00000016,
	CmppPushMoRouteUpdateResp = 0x80000016,
	CmppGetMoRoute = 0x00000017,
	CmppGetMoRouteResp = 0x80000017,
	UnKnow = 0x0,
}

impl CmppType {
	pub fn get_type(id: u32) -> Self {
		match id {
			0x00000004 => CmppType::CmppSubmit,
			0x80000004 => CmppType::CmppSubmitResp,
			0x00000005 => CmppType::CmppDeliver,
			0x80000005 => CmppType::CmppDeliverResp,
			0x00000001 => CmppType::CmppConnect,
			0x80000001 => CmppType::CmppConnectResp,
			0x00000002 => CmppType::CmppTerminate,
			0x80000002 => CmppType::CmppTerminateResp,
			0x00000006 => CmppType::CmppQuery,
			0x80000006 => CmppType::CmppQueryResp,
			0x00000007 => CmppType::CmppCancel,
			0x80000007 => CmppType::CmppCancelResp,
			0x00000008 => CmppType::CmppActiveTest,
			0x80000008 => CmppType::CmppActiveTestResp,
			0x00000009 => CmppType::CmppFwd,
			0x80000009 => CmppType::CmppFwdResp,
			0x00000010 => CmppType::CmppMtRoute,
			0x80000010 => CmppType::CmppMtRouteResp,
			0x00000011 => CmppType::CmppMoRoute,
			0x80000011 => CmppType::CmppMoRouteResp,
			0x00000012 => CmppType::CmppGetMtRoute,
			0x80000012 => CmppType::CmppGetMtRouteResp,
			0x00000013 => CmppType::CmppMtRouteUpdate,
			0x80000013 => CmppType::CmppMtRouteUpdateResp,
			0x00000014 => CmppType::CmppMoRouteUpdate,
			0x80000014 => CmppType::CmppMoRouteUpdateResp,
			0x00000015 => CmppType::CmppPushMtRouteUpdate,
			0x80000015 => CmppType::CmppPushMtRouteUpdateResp,
			0x00000016 => CmppType::CmppPushMoRouteUpdate,
			0x80000016 => CmppType::CmppPushMoRouteUpdateResp,
			0x00000017 => CmppType::CmppGetMoRoute,
			0x80000017 => CmppType::CmppGetMoRouteResp,
			_ => CmppType::UnKnow,
		}
	}
}

impl Protocol for Cmpp {
	fn login_msg(&self, sp_id: &str, password: &str, version: u8, seq: u32) -> Result<BytesMut, io::Error> {
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
		dst.put_u32(CmppType::CmppConnect as u32);
		dst.put_u32(seq);
		dst.extend_from_slice(sp_id.as_bytes());
		dst.extend_from_slice(auth.as_ref());
		dst.put_u8(version);
		dst.put_u32(time);

		Ok(dst)
	}

	///处理送过来的消息。因为之前使用的解码器是在单线程内。
/// 所以只处理断包和粘包。
/// 这里才是真正处理数据的地方。
/// 到这里应该已经处理掉了长度的头。
	fn decode_read_message(&self, buf: &mut BytesMut) -> Result<JsonValue, Error> {
		//拿command
		let tp = buf.get_u32();
		//拿掉seq
		let seq = buf.get_u32();

		let msg = match CmppType::get_type(tp) {
			CmppType::CmppSubmit |
			CmppType::CmppSubmitResp |
			CmppType::CmppDeliver |
			CmppType::CmppDeliverResp |
			CmppType::CmppActiveTest |
			CmppType::CmppActiveTestResp |
			CmppType::CmppConnect => self.decode_connect(buf, seq)?,
			CmppType::CmppConnectResp => self.decode_connect_resp(buf, seq)?,

			CmppType::CmppTerminate |
			CmppType::CmppTerminateResp |
			CmppType::CmppQuery |
			CmppType::CmppQueryResp |
			CmppType::CmppCancel |
			CmppType::CmppCancelResp |
			CmppType::CmppFwd |
			CmppType::CmppFwdResp |
			CmppType::CmppMtRoute |
			CmppType::CmppMtRouteResp |
			CmppType::CmppMoRoute |
			CmppType::CmppMoRouteResp |
			CmppType::CmppGetMtRoute |
			CmppType::CmppGetMtRouteResp |
			CmppType::CmppMtRouteUpdate |
			CmppType::CmppMtRouteUpdateResp |
			CmppType::CmppMoRouteUpdate |
			CmppType::CmppMoRouteUpdateResp |
			CmppType::CmppPushMtRouteUpdate |
			CmppType::CmppPushMtRouteUpdateResp |
			CmppType::CmppPushMoRouteUpdate |
			CmppType::CmppPushMoRouteUpdateResp |
			CmppType::CmppGetMoRoute |
			CmppType::CmppGetMoRouteResp =>
				return Err(io::Error::new(io::ErrorKind::Other, "还未实现")),
			_ => return Err(io::Error::new(io::ErrorKind::NotFound, "type没值,或者无法转换")),
		};

		Ok(msg)
	}
}

impl Clone for Cmpp {
	fn clone(&self) -> Self {
		Cmpp::new()
	}
}

impl Cmpp {
	pub fn new() -> Cmpp {
		Cmpp {
			codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}

	fn decode_connect_resp(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, Error> {
		let mut json = JsonValue::new_object();
		json["Sequence_Id"] = seq.into();
		json["Status"] = buf.get_u32().into();
		buf.advance(16);
		json["Version"] = buf.get_u8().into();

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32) -> Result<JsonValue, Error> {
		info!("现在还未实现.{:?},seq:{}", buf, seq);
		Ok(JsonValue::new_object())
	}
}

impl Encoder<BytesMut> for Cmpp {
	type Error = Error;

	fn encode(&mut self, item: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
		dst.extend_from_slice(&*item);

		Ok(())
	}
}

impl Decoder for Cmpp {
	type Item = BytesMut;
	type Error = Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		self.codec.decode(src)
	}

}
