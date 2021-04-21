use bytes::{BytesMut, Buf, Bytes, BufMut};
use encoding::all::{UTF_16BE, GBK};
use encoding::{Encoding, DecoderTrap, EncoderTrap};
use crate::global::get_sequence_id;
use chrono::{Datelike, DateTime, Local, Timelike};
use json::JsonValue;
use tokio::io;

use crate::global::ISMG_ID;
use crate::global::FILL_ZERO;
use crate::protocol::msg_type::MsgType::{Connect, SubmitResp};
use tokio::io::Error;
use crate::protocol::msg_type::SmsStatus;
use crate::protocol::names::{SEQ_ID, AUTHENTICATOR, VERSION, STATUS, MSG_TYPE_U32, MSG_CONTENT, MSG_ID, SERVICE_ID, TP_UDHI, SP_ID, VALID_TIME, AT_TIME, SRC_ID, MSG_FMT, DEST_IDS, RESULT, DEST_ID, STATE, SUBMIT_TIME, DONE_TIME, SMSC_SEQUENCE, IS_REPORT, MSG_TYPE_STR, LONG_SMS_TOTAL, LONG_SMS_NOW_NUMBER, SEQ_IDS, LOGIN_NAME, PASSWORD, TIMESTAMP};
use crate::protocol::MsgType;
use std::ops::Add;

pub trait ProtocolImpl: Send + Sync {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>>;

	///解码送过来的消息。
	fn decode_read_msg(&mut self, buf: &mut BytesMut) -> io::Result<Option<JsonValue>> {
		//先处理粘包和断包
		match self.get_framed(buf) {
			//这里直接用
			Ok(Some(mut buf)) => {
				log::trace!("收到的消息. src:{:X}", buf);
				//拿command
				let tp = buf.get_u32();
				//拿掉seq.对sgip来说.这里只是拿了node_id.还有2个没拿
				let seq = buf.get_u32();

				let mut msg_type = self.get_type_enum(tp);
				let mut msg = match msg_type {
					MsgType::Submit => self.decode_submit(&mut buf, seq, tp)?,
					MsgType::DeliverResp => self.decode_submit_or_deliver_resp(&mut buf, seq, tp)?,
					MsgType::SubmitResp => self.decode_submit_or_deliver_resp(&mut buf, seq, tp)?,
					MsgType::Deliver => {
						let json = self.decode_deliver(&mut buf, seq, tp)?;
						if let Some(true) = json[IS_REPORT].as_bool() {
							msg_type = MsgType::Report;
						}

						json
					}
					MsgType::ActiveTest | MsgType::ActiveTestResp => self.decode_nobody(&mut buf, seq, tp)?,
					MsgType::Connect => self.decode_connect(&mut buf, seq, tp)?,
					MsgType::ConnectResp => self.decode_connect_resp(&mut buf, seq, tp)?,
					MsgType::Terminate | MsgType::TerminateResp => self.decode_nobody(&mut buf, seq, tp)?,
					MsgType::Report => self.decode_report(&mut buf, seq, tp)?,
					MsgType::ReportResp => self.decode_report_resp(&mut buf, seq, tp)?,
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
					MsgType::UNKNOWN |
					MsgType::GetMoRouteResp =>
						return Err(io::Error::new(io::ErrorKind::Other, "还未实现")),
					// _ => return Err(io::Error::new(io::ErrorKind::NotFound, "type没值,或者无法转换")),
				};

				//这句是把对应的消息转成字符串用来处理。因为id每一个协议不一样。而且枚举在json结构里面又不支持。
				let msg_type_str: &str = msg_type.into();
				msg[MSG_TYPE_STR] = msg_type_str.into();
				buf.truncate(0);

				Ok(Some(msg))
			}
			Ok(None) => Ok(None),
			//当出现异常时这里直接返回空.让上层应用不进行处理.
			Err(_) => Ok(None)
		}
	}

	fn get_type_id(&self, t: MsgType) -> u32 {
		match t {
			MsgType::Submit => 0x00000004,
			MsgType::SubmitResp => 0x80000004,
			MsgType::Deliver => 0x00000005,
			MsgType::DeliverResp => 0x80000005,
			MsgType::Report => 0x01000005,
			MsgType::ReportResp => 0x81000005,
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
			MsgType::UNKNOWN => 0,
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
			_ => MsgType::UNKNOWN,
		}
	}

	fn get_status_id(&self, status: &SmsStatus) -> u32 {
		match status {
			SmsStatus::Success => 0,
			SmsStatus::MessageError => 1,
			SmsStatus::AddError => 2,
			SmsStatus::AuthError => 3,
			SmsStatus::VersionError => 4,
			SmsStatus::TrafficRestrictions => 8,
			SmsStatus::OtherError => 5,
			SmsStatus::UNKNOWN => 999,
		}
	}

	fn get_status_enum(&self, v: u32) -> SmsStatus {
		match v {
			0 => SmsStatus::Success,
			1 => SmsStatus::MessageError,
			2 => SmsStatus::AddError,
			3 => SmsStatus::AuthError,
			4 => SmsStatus::VersionError,
			8 => SmsStatus::TrafficRestrictions,
			5 => SmsStatus::OtherError,
			_ => { SmsStatus::OtherError }
		}
	}

	///通过给定的账号密码。计算实际向客户发送的消息，也是用来进行校验密码是否正确。
	fn get_auth(&self, sp_id: &str, password: &str, timestamp: u32) -> [u8; 16] {
		let time_str = format!("{:010}", timestamp);

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

		md5::compute(src).0
	}

	///生成登录操作的消息
	fn encode_connect(&self, json: &mut JsonValue) -> Result<BytesMut, io::Error> {
		let sp_id = match json[LOGIN_NAME].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有login_name.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有login_name"));
			}
		};

		let password = match json[PASSWORD].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有password.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有password"));
			}
		};

		let version = match json[VERSION].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有version.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有version"));
			}
		};

		if sp_id.len() != 6 {
			return Err(io::Error::new(io::ErrorKind::Other, "sp_id,长度应该为6位"));
		}

		let time = get_time();
		//固定这个大小。
		let mut dst = BytesMut::with_capacity(39);
		dst.put_u32(39);
		dst.put_u32(self.get_type_id(Connect));
		dst.put_u32(get_sequence_id(1));
		dst.extend_from_slice(sp_id.as_bytes());
		dst.extend_from_slice(&self.get_auth(sp_id, password, time));
		dst.put_u8(version as u8);
		dst.put_u32(time);

		Ok(dst)
	}

	///根据对方给的请求,处理以后的编码消息
	fn encode_connect_rep(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let mut dst = BytesMut::with_capacity(33);
		dst.put_u32(33);
		dst.put_u32(self.get_type_id(MsgType::ConnectResp));

		dst.put_u32(json[SEQ_ID].as_u32().unwrap());
		dst.put_u32(self.get_status_id(&status));

		dst.extend_from_slice(&FILL_ZERO[0..16]);
		dst.put_u8(json[VERSION].as_u8().unwrap());

		Some(dst)
	}

	fn encode_submit_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let dest_ids = if json[DEST_IDS].is_array() {
			let mut dest_ids = Vec::with_capacity(json[DEST_IDS].len());
			json[DEST_IDS].members().for_each(|item| dest_ids.push(item.as_str().unwrap()));

			dest_ids
		} else {
			log::error!("没有dest_ids.退出..json:{}", json);
			return None;
		};

		let submit_status = self.get_status_id(&status);
		let msg_id: u64 = create_cmpp_msg_id(dest_ids.len() as u32);

		let mut buf = BytesMut::with_capacity(24);
		buf.put_u32(24);
		buf.put_u32(self.get_type_id(SubmitResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u32(submit_status);

		json[MSG_ID] = cmpp_msg_id_u64_to_str(msg_id).into();
		Some(buf)
	}

	fn encode_report_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return None;
			}
			Some(v) => cmpp_msg_id_str_to_u64(v)
		};

		let status = self.get_status_id(&status);

		let mut buf = BytesMut::with_capacity(24);

		buf.put_u32(24);
		buf.put_u32(self.get_type_id(MsgType::DeliverResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u32(status);

		Some(buf)
	}

	fn encode_deliver_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let seq_id = match json[SEQ_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有seq_id.退出..json:{}", json);
				return None;
			}
		};

		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return None;
			}
			Some(v) => cmpp_msg_id_str_to_u64(v)
		};

		let status = self.get_status_id(&status);

		let mut buf = BytesMut::with_capacity(24);

		buf.put_u32(24);
		buf.put_u32(self.get_type_id(MsgType::DeliverResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u32(status);

		Some(buf)
	}

	fn encode_active_test_resp(&self, _status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
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

	fn encode_active_test(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let mut dst = BytesMut::with_capacity(12);

		dst.put_u32(12);
		dst.put_u32(self.get_type_id(MsgType::ActiveTest));
		let seq_id = get_sequence_id(1);
		dst.put_u32(seq_id);

		json[SEQ_ID] = seq_id.into();
		Ok(dst)
	}

	fn encode_terminate(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let mut dst = BytesMut::with_capacity(12);

		dst.put_u32(12);
		dst.put_u32(self.get_type_id(MsgType::Terminate));
		let seq_id = get_sequence_id(1);
		dst.put_u32(seq_id);

		json[SEQ_ID] = seq_id.into();
		Ok(dst)
	}

	fn encode_report(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => cmpp_msg_id_str_to_u64(v)
		};

		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有service_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有service_id"));
			}
		};

		let stat = match json[STATE].as_str() {
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
		let mut dst = BytesMut::with_capacity(180);

		dst.put_u32(180);
		dst.put_u32(self.get_type_id(MsgType::Deliver));
		let mut seq_ids = Vec::with_capacity(1);
		let seq_id = get_sequence_id(1);
		seq_ids.push(seq_id);
		dst.put_u32(seq_id);

		dst.put_u64(create_cmpp_msg_id(1)); //Msg_Id 8
		fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21
		fill_bytes_zero(&mut dst, service_id, 10);//service_id 10
		dst.put_u8(0); //TP_pid 1
		dst.put_u8(0); //tp_udhi 1
		dst.put_u8(0); //Msg_Fmt 1
		fill_bytes_zero(&mut dst, src_id, 32);  //src_id 32
		dst.put_u8(0); //Src_terminal_type 1
		dst.put_u8(1); //Registered_Delivery 1 1:状态报告
		dst.put_u8(71); //Msg_Length 状态报告长度固定

		dst.put_u64(msg_id); // Msg_Id 状态报告的msg_id 指哪一条的msg_id
		fill_bytes_zero(&mut dst, stat, 7); //Stat 7
		fill_bytes_zero(&mut dst, submit_time, 10); //submit_time 10
		fill_bytes_zero(&mut dst, done_time, 10); //done_time 10
		fill_bytes_zero(&mut dst, src_id, 32); //Dest_terminal_Id 32
		dst.put_u32(smsc_sequence);//SMSC_SEQUENCE 4
		dst.extend_from_slice(&FILL_ZERO[0..20]); //LinkID

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}

	fn encode_deliver(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => cmpp_msg_id_str_to_u64(v)
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
		//109是除开内容\发送号码之后所有长度加在一起
		let total_len = sms_len * (109 + msg_content_head_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((109 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Deliver));
			let seq_id = get_sequence_id(1);
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);

			dst.put_u64(msg_id); //Msg_Id 8
			fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21
			fill_bytes_zero(&mut dst, service_id, 10);//service_id 10
			dst.put_u8(0); //TP_pid 1
			dst.put_u8(if sms_len == 1 { 0 } else { 1 }); //tp_udhi 1
			dst.put_u8(8); //Msg_Fmt 1
			fill_bytes_zero(&mut dst, src_id, 32);  //src_id 32
			dst.put_u8(0); //Src_terminal_type 1
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
			dst.extend_from_slice(&FILL_ZERO[0..20]); //LinkID
		}

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}

	fn encode_submit(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
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
			None => ""
		};

		let at_time = match json[AT_TIME].as_str() {
			Some(v) => v,
			None => ""
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

		let seq_id = match json[SEQ_ID].as_u64() {
			None => {
				get_sequence_id(dest_ids.len() as u32)
			}
			Some(v) => v as u32
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
		//163是除开内容\发送号码之后所有长度加在一起 6是长短信消息头长度
		let total_len = sms_len * (163 + dest_ids.len() * 32 + msg_content_head_len) + msg_content_len;

		// 163 + msg_content_len + dest_ids.len() * 32;
		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((163 + dest_ids.len() * 32 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Submit));
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);

			dst.put_u64(0); //Msg_Id 8
			dst.put_u8(1); //Pk_total
			dst.put_u8(1); //Pk_number
			dst.put_u8(1); //Registered_Delivery
			dst.put_u8(1); //Msg_level
			fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
			dst.put_u8(3); //Fee_UserType
			dst.extend_from_slice(&FILL_ZERO[0..32]);//Fee_terminal_Id
			dst.put_u8(0); //Fee_terminal_type
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
			dest_ids.iter().for_each(|dest_id| fill_bytes_zero(&mut dst, dest_id, 32));  //dest_id

			dst.put_u8(0); //Dest_terminal_type
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
			dst.extend_from_slice(&FILL_ZERO[0..20]); //LinkID
		}

		json[SEQ_IDS] = seq_ids.into();

		Ok(dst)
	}

	fn decode_connect_resp(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		//21是已经减掉头的长度.cmpp3.0是21
		if buf.len() < 21 {
			//cmpp 2.0
			json[STATUS] = buf.get_u8().into();
		} else {
			json[STATUS] = buf.get_u32().into();
		}

		buf.advance(16);
		json[VERSION] = buf.get_u8().into();
		Ok(json)
	}

	fn decode_submit_or_deliver_resp(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();
		json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into();//msg_id 8
		json[RESULT] = buf.get_u32().into();//result 4

		Ok(json)
	}

	fn decode_nobody(&self, _buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		Ok(json)
	}

	fn decode_report(&self, _buf: &mut BytesMut, _seq: u32, _tp: u32) -> Result<JsonValue, io::Error> {
		log::error!("此协议应该不会收到这个消息ID");
		Err(io::Error::new(io::ErrorKind::InvalidData, "此协议应该不会收到这个消息ID"))
	}

	fn decode_deliver(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into(); //msg_id 8
		json[DEST_ID] = load_utf8_string(buf, 21).into(); //dest_id 21
		json[SERVICE_ID] = load_utf8_string(buf, 10).into(); //service_id 10
		buf.advance(1); //TP_pid 1
		let tp_udhi = buf.get_u8(); //是否长短信
		let msg_fmt = buf.get_u8();
		json[SRC_ID] = load_utf8_string(buf, 32).into(); //src_id 32
		buf.advance(1); //Src_terminal_type	1

		let is_report = buf.get_u8(); //Registered_Delivery	1
		if is_report == 0 {
			//长短信的处理 tp_udhi != 0 说明是长短信
			json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
			let msg_content_len = buf.get_u8(); //Msg_Length	1
			decode_msg_content(buf, msg_fmt, msg_content_len, &mut json, tp_udhi != 0)?;
		} else {
			buf.advance(1); //Msg_Length 1
			json[IS_REPORT] = true.into(); //状态报告增加.
			json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into(); // Msg_Id
			json[STATE] = load_utf8_string(buf, 7).into(); // Stat
			json[SUBMIT_TIME] = load_utf8_string(buf, 10).into(); // Submit_time
			json[DONE_TIME] = load_utf8_string(buf, 10).into(); // Done_time
			json[SRC_ID] = load_utf8_string(buf, 32).into(); // dest_terminal_id

			//是状态报告.修改一下返回的类型.
			json[MSG_TYPE_U32] = self.get_type_id(MsgType::Report).into();
		}

		Ok(json)
	}

	fn decode_report_resp(&self, _buf: &mut BytesMut, _seq: u32, _tp: u32) -> Result<JsonValue, io::Error> {
		log::error!("此协议应该不会收到这个消息ID");
		Err(io::Error::new(io::ErrorKind::InvalidData, "此协议应该不会收到这个消息ID"))
	}

	fn decode_submit(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into(); //msg_id 8
		buf.advance(4); //Pk_total 1 Pk_number 1 Registered_Delivery 1 Msg_level 1
		json[SERVICE_ID] = load_utf8_string(buf, 10).into(); //service_id 10
		buf.advance(35); //Fee_UserType 1  Fee_terminal_Id 32 Fee_terminal_type	1 TP_pId	1
		let tp_udhi = buf.get_u8(); //是否长短信
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
		json[SP_ID] = load_utf8_string(buf, 6).into(); //sp_id 6
		buf.advance(8); //FeeType	2  FeeCode	6
		json[VALID_TIME] = load_utf8_string(buf, 17).into(); //valid_time 17
		json[AT_TIME] = load_utf8_string(buf, 17).into(); //at_time 17
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		let dest_len = buf.get_u8(); //DestUsr_tl 1

		//dest_id 32
		let mut dest_ids: Vec<String> = Vec::new();
		for _i in 0..dest_len {
			dest_ids.push(load_utf8_string(buf, 32));
		}

		json[DEST_IDS] = dest_ids.into();

		buf.advance(1); //Dest_terminal_type	1

		//长短信的处理 tp_udhi != 0 说明是长短信
		let msg_content_len = buf.get_u8(); //Msg_Length	1
		decode_msg_content(buf, msg_fmt, msg_content_len, &mut json, tp_udhi != 0)?;

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();
		json[LOGIN_NAME] = load_utf8_string(buf, 6).into();
		//现在取2次,相当于只保存8位.
		json[AUTHENTICATOR] = buf.get_u64().into();
		json[AUTHENTICATOR] = buf.get_u64().into();
		json[VERSION] = (buf.get_u8() as u32).into();
		json[TIMESTAMP] = buf.get_u32().into();

		Ok(json)
	}
}

///向缓冲写入指定长度数据.如果数据不够长.使用0进行填充.
pub fn fill_bytes_zero(dest: &mut BytesMut, slice: &str, len: usize) {
	if slice.len() >= len {
		dest.extend_from_slice(slice[0..len].as_bytes());
	} else {
		dest.extend_from_slice(slice.as_bytes());
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

//TODO 这里需要进行些修改
pub fn smgp_msg_id_str_to_u64(msg_id: &str) -> (u16,u64) {
	(0,msg_id.parse().unwrap_or(0))
}

//TODO 这里需要进行些修改
pub fn smgp_msg_id_u64_to_str(ismg_id: u16,  msg_id: u64) -> String {
	ismg_id.to_string().add(msg_id.to_string().as_str())
}

pub fn cmpp_msg_id_u64_to_str(mut msg_id: u64) -> String {
	let mut us = vec![0x30u8; 21];

	let mut d = msg_id & 0xffff;
	let mut ind = 20;
	for _ in 0..5 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 16;

	d = msg_id & 0x3FFFFF;
	for _ in 0..6 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 22;

	d = msg_id & 0x3F;
	for _ in 0..2 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 6;

	d = msg_id & 0x3F;
	for _ in 0..2 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 6;

	d = msg_id & 0x1F;
	for _ in 0..2 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 5;

	d = msg_id & 0x1F;
	for _ in 0..2 {
		us[ind] = 0x30 + (d % 10) as u8;
		d = d / 10;
		ind -= 1;
	}
	msg_id = msg_id >> 5;

	d = msg_id & 0x1F;
	us[ind] = 0x30 + (d % 10) as u8;
	d = d / 10;
	ind -= 1;
	us[ind] = 0x30 + (d % 10) as u8;

	let res = unsafe {
		String::from_utf8_unchecked(us)
	};

	res
}

pub fn cmpp_msg_id_str_to_u64(msg_id: &str) -> u64 {
	// if !msg_id.as_bytes().is_ascii() {
	//
	// }
	let (month, dd) = msg_id.split_at(2);
	let month: u64 = month.parse().unwrap_or(0);
	let (day, dd) = dd.split_at(2);
	let day: u64 = day.parse().unwrap_or(0);
	let (hour, dd) = dd.split_at(2);
	let hour: u64 = hour.parse().unwrap_or(0);
	let (minute, dd) = dd.split_at(2);
	let minute: u64 = minute.parse().unwrap_or(0);
	let (second, dd) = dd.split_at(2);
	let second: u64 = second.parse().unwrap_or(0);
	let (ismg_id, seq_id) = dd.split_at(6);
	let ismg_id: u64 = ismg_id.parse().unwrap_or(0);
	let seq_id: u64 = seq_id.parse().unwrap_or(0);

	let mut result = month;
	result = result << 5 | (day & 0x1F);
	result = result << 5 | (hour & 0x1F) as u64;
	result = result << 6 | (minute & 0x3F) as u64;
	result = result << 6 | (second & 0x3F) as u64;
	result = result << 22 | ismg_id as u64;

	result << 16 | seq_id
}

///返回一个msg_id.目前在Cmpp协议里面使用
pub fn create_cmpp_msg_id(len: u32) -> u64 {
	let date = Local::now();
	let mut result: u64 = date.month() as u64;
	result = result << 5 | (date.day() & 0x1F) as u64;
	result = result << 5 | (date.hour() & 0x1F) as u64;
	result = result << 6 | (date.minute() & 0x3F) as u64;
	result = result << 6 | (date.second() & 0x3F) as u64;
	result = result << 22 | *ISMG_ID as u64;

	result << 16 | (get_sequence_id(len) & 0xffff) as u64
}

///处理短信内容的通用方法.msg_content..
pub fn decode_msg_content(buf: &mut BytesMut, msg_fmt: u8, mut msg_content_len: u8, json: &mut JsonValue, is_long_sms: bool) -> Result<(), io::Error> {
	if is_long_sms {
		let head_len = buf.get_u8(); //接下来的长度
		buf.advance((head_len - 2) as usize); //跳过对应的长度
		json[LONG_SMS_TOTAL] = buf.get_u8().into();
		json[LONG_SMS_NOW_NUMBER] = buf.get_u8().into();
		json[TP_UDHI] = 1.into(); //TP_udhi 1

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
		15 =>  match GBK.decode(&msg_content, DecoderTrap::Strict) {
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

///得到一个mmddhhMMss格式的当前时间
pub fn get_time() -> u32 {
	let local: DateTime<Local> = Local::now();
	let mut time: u32 = local.month();
	time = time * 100 + local.day();
	time = time * 100 + local.hour();
	time = time * 100 + local.minute();
	time = time * 100 + local.second();

	time
}
