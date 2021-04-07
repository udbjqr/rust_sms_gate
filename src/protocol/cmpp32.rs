use bytes::{Buf, BufMut, BytesMut};
use chrono::{Datelike, DateTime, Local, Timelike};
use json::JsonValue;
use tokio::io;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::protocol::{MsgType, Protocol, fill_bytes_zero, load_utf8_string, get_msg_id, decode_msg_content, copy_to_bytes};
use crate::protocol::msg_type::MsgType::{Connect, SubmitResp};
use tokio::io::Error;
use crate::global::{get_sequence_id, FILL_ZERO};
use crate::protocol::msg_type::SmsStatus;
use crate::protocol::names::{SEQ_ID, AUTHENTICATOR, VERSION, STATUS, MSG_TYPE, USER_ID, MSG_CONTENT, MSG_ID, SERVICE_ID, TP_UDHI, SP_ID, VALID_TIME, AT_TIME, SRC_ID, MSG_FMT, DEST_IDS, RESULT, DEST_ID, STAT, SUBMIT_TIME, DONE_TIME, SMSC_SEQUENCE, IS_REPORT, MSG_TYPE_STR};
use encoding::all::UTF_16BE;
use encoding::{Encoding, EncoderTrap};

///CMPP协议2.0的处理
#[derive(Debug, Default)]
pub struct Cmpp32 {
	version: u32,
	codec: LengthDelimitedCodec,
}

impl Protocol for Cmpp32 {
	fn login_rep(&self, status: u32, json: &JsonValue) -> BytesMut {
		let mut dst = BytesMut::with_capacity(30);
		dst.put_u32(30);
		dst.put_u32(self.get_type_id(MsgType::ConnectResp));

		dst.put_u32(json[SEQ_ID].as_u32().unwrap());
		dst.put_u8(status as u8);

		let str = json[AUTHENTICATOR].as_str().unwrap().as_bytes();

		dst.extend_from_slice(&str[0..16]);
		dst.put_u8(json[VERSION].as_u8().unwrap());

		dst
	}

	fn encode_receipt(&self, status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut> {
		match self.get_type_enum(json[MSG_TYPE].as_u32().unwrap()) {
			MsgType::Submit => self.encode_submit_resp(status, json),
			MsgType::Deliver => self.encode_deliver_resp(status, json),
			MsgType::ActiveTest => self.encode_active_test_resp(status, json),
			_ => {
				log::error!("未生成的返回对象.json:{}", json);
				None
			}
		}
	}

	fn get_version(&self) -> u32 {
		self.version
	}

	fn encode_from_entity_message(&self, json: &JsonValue) -> Result<(BytesMut, Vec<u32>), Error> {
		match json[MSG_TYPE].as_u32() {
			None => Err(Error::new(io::ErrorKind::NotFound, format!("没有找到指定的type.json:{}", json))),
			Some(msg_type) => {
				match self.get_type_enum(msg_type) {
					MsgType::Submit => self.encode_submit(json),
					MsgType::Deliver => self.encode_deliver(json),
					MsgType::Report => self.encode_report(json),
					MsgType::ActiveTest => self.encode_active_test(json),
					_ => Err(io::Error::new(io::ErrorKind::Other, "还未实现")),
				}
			}
		}
	}


	fn encode_login_msg(&self, sp_id: &str, password: &str) -> Result<BytesMut, io::Error> {
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
		dst.put_u8(32);
		dst.put_u32(time);

		Ok(dst)
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

		let msg_type = self.get_type_enum(tp);
		let mut msg = match msg_type {
			MsgType::Submit => self.decode_submit(buf, seq, tp)?,
			MsgType::DeliverResp | MsgType::SubmitResp => self.decode_submit_or_deliver_resp(buf, seq, tp)?,
			MsgType::Deliver => self.decode_deliver(buf, seq, tp)?,
			MsgType::ActiveTest | MsgType::ActiveTestResp => self.decode_nobody(buf, seq, tp)?,
			MsgType::Connect => self.decode_connect(buf, seq, tp)?,
			MsgType::ConnectResp => self.decode_connect_resp(buf, seq, tp)?,
			MsgType::Terminate | MsgType::TerminateResp => self.decode_nobody(buf, seq, tp)?,
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

		//这句是把对应的消息转成字符串用来处理。因为id每一个协议不一样。而且枚举在json结构里面又不支持。
		let msg_type_str: &str = msg_type.into();
		msg[MSG_TYPE_STR] = msg_type_str.into();

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
			//这两个特殊.是自己定义的.因为cmpp消息里面都是使用deliver来传状态报告
			0x01000005 => MsgType::Report,
			0x81000005 => MsgType::ReportResp,
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

impl Clone for Cmpp32 {
	fn clone(&self) -> Self {
		Cmpp32::new()
	}
}

impl Cmpp32 {
	pub fn new() -> Self {
		Cmpp32 {
			version: 32,
			codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}

	fn encode_submit_resp(&self, status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let submit_status = self.get_status_id(&status);

		let msg_id: u64 = get_msg_id();
		json[MSG_ID] = msg_id.into();

		let mut buf = BytesMut::with_capacity(21);

		buf.put_u32(21);
		buf.put_u32(self.get_type_id(SubmitResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u8(submit_status as u8);

		Some(buf)
	}

	fn encode_deliver_resp(&self, status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let msg_id = match json[MSG_ID].as_u64() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return None;
			}
			Some(v) => v
		};

		let status = self.get_status_id(&status);

		let mut buf = BytesMut::with_capacity(21);

		buf.put_u32(21);
		buf.put_u32(self.get_type_id(MsgType::DeliverResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u8(status as u8);

		Some(buf)
	}

	fn encode_active_test_resp(&self, _status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let mut buf = BytesMut::with_capacity(12);

		buf.put_u32(12);
		buf.put_u32(self.get_type_id(MsgType::ActiveTestResp));
		buf.put_u32(seq_id);

		Some(buf)
	}

	fn encode_active_test(&self, _json: &JsonValue) -> Result<(BytesMut, Vec<u32>), Error> {
		let mut dst = BytesMut::with_capacity(12);

		dst.put_u32(12);
		dst.put_u32(self.get_type_id(MsgType::ActiveTest));
		let mut seq_ids = Vec::with_capacity(1);
		let seq_id = get_sequence_id(1);
		seq_ids.push(seq_id);
		dst.put_u32(seq_id);

		Ok((dst, seq_ids))
	}

	fn encode_report(&self, json: &JsonValue) -> Result<(BytesMut, Vec<u32>), Error> {
		//只检测一下.如果没有后续不处理.
		let msg_id = match json[MSG_ID].as_u64() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => v
		};
		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有service_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有service_id"));
			}
		};

		let stat = match json[STAT].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有stat.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有stat"));
			}
		};

		let submit_time = match json[SUBMIT_TIME].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有submit_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有submit_time"));
			}
		};

		let done_time = match json[DONE_TIME].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有done_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有done_time"));
			}
		};

		let smsc_sequence = match json[SMSC_SEQUENCE].as_u32() {
			Some(v) => v,
			None => 0
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


		//状态报告长度固定
		let mut dst = BytesMut::with_capacity(133);

		dst.put_u32(133);
		dst.put_u32(self.get_type_id(MsgType::Deliver));
		let mut seq_ids = Vec::with_capacity(1);
		let seq_id = get_sequence_id(1);
		seq_ids.push(seq_id);
		dst.put_u32(seq_id);

		dst.put_u64(get_msg_id()); //Msg_Id 8
		fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21
		fill_bytes_zero(&mut dst, service_id, 10);//service_id 10
		dst.put_u8(0); //TP_pid 1
		dst.put_u8(0); //tp_udhi 1
		dst.put_u8(0); //Msg_Fmt 1
		fill_bytes_zero(&mut dst, src_id, 21);  //src_id 21
		dst.put_u8(1); //Registered_Delivery 1 1:状态报告
		dst.put_u8(60); //Msg_Length 状态报告长度固定
		dst.put_u64(msg_id); // Msg_Id 状态报告的msg_id 指哪一条的msg_id
		fill_bytes_zero(&mut dst, stat, 7); //Stat 7
		fill_bytes_zero(&mut dst, submit_time, 10); //submit_time 10
		fill_bytes_zero(&mut dst, done_time, 10); //done_time 10
		fill_bytes_zero(&mut dst, src_id, 21); //Dest_terminal_Id 21
		dst.put_u32(smsc_sequence);//SMSC_SEQUENCE 4
		dst.extend_from_slice(&FILL_ZERO[0..8]); //Reserved

		Ok((dst, seq_ids))
	}

	fn encode_deliver(&self, json: &JsonValue) -> Result<(BytesMut, Vec<u32>), Error> {
		//只检测一下.如果没有后续不处理.
		let _msg_id = match json[MSG_ID].as_u64() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => v
		};

		let msg_content = match json[MSG_CONTENT].as_str() {
			None => {
				log::error!("没有内容字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有内容字串"));
			}
			Some(v) => v
		};

		//编码以后的消息内容
		let msg_content_code = match UTF_16BE.encode(msg_content, EncoderTrap::Strict) {
			Ok(v) => v,
			Err(e) => {
				log::error!("字符串内容解码出现错误..json:{}.e:{}", json, e);
				return Err(io::Error::new(io::ErrorKind::Other, "字符串内容解码出现错误"));
			}
		};

		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有service_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有service_id"));
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

		let msg_content_len = msg_content_code.len();

		let mut msg_content_head_len: usize = 0;
		let mut one_content_len: usize = 140;
		let mut msg_content_seq_id: u8 = 0;
		//整个消息的长度
		let sms_len = if msg_content_len <= 140 {
			1 as usize
		} else {
			msg_content_head_len = 6;
			one_content_len = one_content_len - 6;
			msg_content_seq_id = get_sequence_id(1) as u8;

			((msg_content_len as f32) / one_content_len as f32).ceil() as usize
		};

		//长短信的话,一次性生成多条记录
		//73是除开内容\发送号码之后所有长度加在一起
		let total_len = sms_len * (73 + msg_content_head_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((73 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Deliver));
			let seq_id = get_sequence_id(1);
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);

			dst.put_u64(get_msg_id()); //Msg_Id 8
			fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21
			fill_bytes_zero(&mut dst, service_id, 10);//service_id 10
			dst.put_u8(0); //TP_pid 1
			dst.put_u8(if sms_len == 1 { 0 } else { 1 }); //tp_udhi 1
			dst.put_u8(8); //Msg_Fmt 1
			fill_bytes_zero(&mut dst, src_id, 21);  //src_id 21
			dst.put_u8(0); //Registered_Delivery 1 0 非状态报告
			dst.put_u8(this_msg_content.len() as u8 + msg_content_head_len as u8); //Msg_Length
			if msg_content_head_len > 0 {
				dst.put_u8(5);
				dst.put_u8(0);
				dst.put_u8(03);
				dst.put_u8(msg_content_seq_id);
				dst.put_u8(sms_len as u8);
				dst.put_u8((i + 1) as u8);
			}
			dst.extend_from_slice(&this_msg_content[..]); //Msg_Content
			dst.extend_from_slice(&FILL_ZERO[0..8]); //Reserved
		}

		Ok((dst, seq_ids))
	}

	fn encode_submit(&self, json: &JsonValue) -> Result<(BytesMut, Vec<u32>), Error> {
		let msg_content = match json[MSG_CONTENT].as_str() {
			None => {
				log::error!("没有内容字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有内容字串"));
			}
			Some(v) => v
		};

		//编码以后的消息内容
		let msg_content_code = match UTF_16BE.encode(msg_content, EncoderTrap::Strict) {
			Ok(v) => v,
			Err(e) => {
				log::error!("字符串内容解码出现错误..json:{}.e:{}", json, e);
				return Err(io::Error::new(io::ErrorKind::Other, "字符串内容解码出现错误"));
			}
		};

		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有service_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有service_id"));
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

		let dest_ids = if json[DEST_IDS].is_array() {
			let mut dest_ids = Vec::with_capacity(json[DEST_IDS].len());
			json[DEST_IDS].members().for_each(|item| dest_ids.push(item.as_str().unwrap()));

			dest_ids
		} else {
			log::error!("没有dest_ids.退出..json:{}", json);
			return Err(io::Error::new(io::ErrorKind::NotFound, "没有dest_ids"));
		};


		let msg_content_len = msg_content_code.len();

		let mut msg_content_head_len: usize = 0;
		let mut one_content_len: usize = 140;
		let mut msg_content_seq_id: u8 = 0;
		//整个消息的长度
		let sms_len = if msg_content_len <= 140 {
			1 as usize
		} else {
			msg_content_head_len = 6;
			one_content_len = one_content_len - 6;
			msg_content_seq_id = get_sequence_id(1) as u8;
			((msg_content_len as f32) / one_content_len as f32).ceil() as usize
		};

		//长短信的话,一次性生成多条记录
		//136是除开内容\发送号码之后所有长度加在一起 6是长短信消息头长度
		let total_len = sms_len * (136 + dest_ids.len() * 21 + msg_content_head_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((136 + dest_ids.len() * 21 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Submit));
			let seq_id = get_sequence_id(1);
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);

			dst.put_u64(0); //Msg_Id 8
			dst.put_u8(1); //Pk_total
			dst.put_u8(1); //Pk_number
			dst.put_u8(1); //Registered_Delivery
			dst.put_u8(1); //Msg_level
			fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
			dst.put_u8(3); //Fee_UserType
			dst.extend_from_slice(&FILL_ZERO[0..21]);//Fee_terminal_Id
			dst.put_u8(0); //TP_pId
			dst.put_u8(if sms_len == 1 { 0 } else { 1 }); //tp_udhi
			dst.put_u8(8); //Msg_Fmt
			dst.extend_from_slice(sp_id[0..6].as_ref()); //sp_id
			dst.extend_from_slice("01".as_ref()); //FeeType
			dst.extend_from_slice("000001".as_ref()); //FeeCode
			fill_bytes_zero(&mut dst, valid_time, 17);  //valid_time
			fill_bytes_zero(&mut dst, at_time, 17);  //at_time
			fill_bytes_zero(&mut dst, src_id, 21);  //src_id
			dst.put_u8(dest_ids.len() as u8); //DestUsr_tl
			dest_ids.iter().for_each(|dest_id| fill_bytes_zero(&mut dst, dest_id, 21));  //dest_id
			dst.put_u8(this_msg_content.len() as u8 + msg_content_head_len as u8); //Msg_Length
			if msg_content_head_len > 0 {
				dst.put_u8(5);
				dst.put_u8(0);
				dst.put_u8(03);
				dst.put_u8(msg_content_seq_id);
				dst.put_u8(sms_len as u8);
				dst.put_u8((i + 1) as u8);
			}
			dst.extend_from_slice(&this_msg_content[..]); //Msg_Content
			dst.extend_from_slice(&FILL_ZERO[0..8]); //LinkID
		}

		Ok((dst, seq_ids))
	}

	fn decode_connect_resp(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = tp.into();
		json[SEQ_ID] = seq.into();
		json[STATUS] = buf.get_u32().into();
		buf.advance(16);
		json[VERSION] = buf.get_u8().into();

		Ok(json)
	}

	fn decode_submit_or_deliver_resp(&self, buf: &mut BytesMut, _seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE] = tp.into();
		json[MSG_ID] = buf.get_u64().into();//msg_id 8
		json[RESULT] = (buf.get_u8() as u32).into();//result 1

		Ok(json)
	}

	fn decode_nobody(&self, _buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = tp.into();
		json[SEQ_ID] = seq.into();

		Ok(json)
	}

	fn decode_deliver(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = tp.into();
		json[SEQ_ID] = seq.into();

		json[MSG_ID] = buf.get_u64().into(); //msg_id 8
		json[DEST_ID] = load_utf8_string(buf, 21).into(); //dest_id 21
		json[SERVICE_ID] = load_utf8_string(buf, 10).into(); //service_id 10
		buf.advance(1); //TP_pid 1
		let tp_udhi = buf.get_u8(); //是否长短信
		let msg_fmt = buf.get_u8();
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		let is_report = buf.get_u8(); //Registered_Delivery	1
		if is_report == 0 {
			//长短信的处理 tp_udhi != 0 说明是长短信
			json[TP_UDHI] = tp_udhi.into(); //TP_udhi 1
			json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
			decode_msg_content(buf, msg_fmt, &mut json, tp_udhi != 0)?;
		} else {
			buf.advance(1); //Msg_Length 1
			json[IS_REPORT] = true.into(); //状态报告增加.
			json[MSG_ID] = buf.get_u64().into(); // Msg_Id
			json[STAT] = load_utf8_string(buf, 7).into(); // Stat
			json[SUBMIT_TIME] = load_utf8_string(buf, 10).into(); // Submit_time
			json[DONE_TIME] = load_utf8_string(buf, 10).into(); // Done_time
			json[SRC_ID] = load_utf8_string(buf, 21).into(); // dest_terminal_id
		}

		Ok(json)
	}

	fn decode_submit(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = tp.into();
		json[SEQ_ID] = seq.into();

		json[MSG_ID] = buf.get_u64().into(); //msg_id 8
		buf.advance(4); //Pk_total 1 Pk_number 1 Registered_Delivery 1 Msg_level 1
		json[SERVICE_ID] = load_utf8_string(buf, 10).into(); //service_id 10
		buf.advance(23); //Fee_UserType 1  Fee_terminal_Id 21 TP_pId	1
		let tp_udhi = buf.get_u8(); //是否长短信
		json[TP_UDHI] = tp_udhi.into(); //TP_udhi 1
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
		json[SP_ID] = load_utf8_string(buf, 6).into(); //sp_id 6
		buf.advance(8); //FeeType	2  FeeCode	6
		json[VALID_TIME] = load_utf8_string(buf, 17).into(); //valid_time 17
		json[AT_TIME] = load_utf8_string(buf, 17).into(); //at_time 17
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		let dest_len = buf.get_u8(); //DestUsr_tl 1

		//dest_id 21
		let mut dest_ids: Vec<String> = Vec::new();
		for _i in 0..dest_len {
			dest_ids.push(load_utf8_string(buf, 21));
		}

		json[DEST_IDS] = dest_ids.into();

		//长短信的处理 tp_udhi != 0 说明是长短信
		decode_msg_content(buf, msg_fmt, &mut json, tp_udhi != 0)?;

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE] = tp.into();
		json[SEQ_ID] = seq.into();
		json[USER_ID] = load_utf8_string(buf, 6).into();
		json[AUTHENTICATOR] = copy_to_bytes(buf, 16).as_ref().into();
		json[VERSION] = buf.get_u8().into();

		Ok(json)
	}
}

impl Encoder<BytesMut> for Cmpp32 {
	type Error = io::Error;

	fn encode(&mut self, item: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
		dst.extend_from_slice(&*item);

		Ok(())
	}
}

impl Decoder for Cmpp32 {
	type Item = JsonValue;
	type Error = io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		match self.codec.decode(src) {
			Ok(Some(mut src)) => {
				self.decode_read_msg(&mut src)
			}
			Ok(None) => Ok(None),
			//当出现异常时这里直接返回空.让上层应用不进行处理.
			Err(_) => Ok(None)
		}
	}
}
