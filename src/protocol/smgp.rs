use tokio_util::codec::{LengthDelimitedCodec, Decoder};
use crate::protocol::implements::{ProtocolImpl, get_time, fill_bytes_zero, load_utf8_string, decode_msg_content, smgp_msg_id_buf_to_str, smgp_msg_id_str_to_buf};
use bytes::{BytesMut, BufMut, Buf};
use tokio::io;
use crate::protocol::{SmsStatus, MsgType};
use json::JsonValue;
use crate::protocol::names::{LOGIN_NAME,IS_REPORT,PASSAGE_MSG_ID, PASSWORD, VERSION, MSG_TYPE_U32, SEQ_ID, AUTHENTICATOR, MSG_CONTENT, SERVICE_ID, VALID_TIME, AT_TIME, SRC_ID, DEST_IDS, SEQ_IDS, MSG_FMT, MSG_ID, RESULT, DEST_ID, SMGP_RECEIVE_TIME, SUBMIT_TIME, DONE_TIME, STATE, ERROR_CODE, TIMESTAMP, MSG_IDS};
use crate::protocol::msg_type::MsgType::{Connect, SubmitResp};
use std::io::Error;
use crate::global::{get_sequence_id, FILL_ZERO};
use crate::global::ISMG_ID;

use super::implements::encode_msg_content;
use super::names::SPEED_LIMIT;

///Sgip协议的处理
#[derive(Debug, Default)]
pub struct Smgp30 {
	version: u32,
	length_codec: LengthDelimitedCodec,
}


impl ProtocolImpl for Smgp30 {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>> {
		self.length_codec.decode(buf)
	}

	fn get_type_id(&self, t: MsgType) -> u32 {
		match t {
			MsgType::Submit => 0x00000002,
			MsgType::SubmitResp => 0x80000002,
			MsgType::Deliver => 0x00000003,
			MsgType::DeliverResp => 0x80000003,
			MsgType::Report => 0x00000005,
			MsgType::ReportResp => 0x80000005,
			MsgType::ActiveTest => 0x00000004,
			MsgType::ActiveTestResp => 0x80000004,
			MsgType::Connect => 0x00000001,
			MsgType::ConnectResp => 0x80000001,
			MsgType::Terminate => 0x00000006,
			MsgType::TerminateResp => 0x80000006,
			_ => 0,
		}
	}

	fn get_type_enum(&self, v: u32) -> MsgType {
		match v {
			0x00000002 => MsgType::Submit,
			0x80000002 => MsgType::SubmitResp,
			0x00000003 => MsgType::Deliver,
			0x80000003 => MsgType::DeliverResp,
			0x00000005 => MsgType::Report,
			0x80000005 => MsgType::ReportResp,
			0x00000004 => MsgType::ActiveTest,
			0x80000004 => MsgType::ActiveTestResp,
			0x00000001 => MsgType::Connect,
			0x80000001 => MsgType::ConnectResp,
			0x00000006 => MsgType::Terminate,
			0x80000006 => MsgType::TerminateResp,
			_ => MsgType::UNKNOWN,
		}
	}


	fn get_status_id(&self, status: &SmsStatus) -> u32 {
		match status {
			SmsStatus::Success => 0,
			SmsStatus::MessageError => 3,
			SmsStatus::AddError => 2,
			SmsStatus::AuthError => 21,
			SmsStatus::VersionError => 29,
			SmsStatus::TrafficRestrictions => 134,
			SmsStatus::OtherError => 5,
			SmsStatus::UNKNOWN => 99,
		}
	}

	fn get_status_enum(&self, v: u32) -> SmsStatus {
		match v {
			0 => SmsStatus::Success,
			3 => SmsStatus::MessageError,
			2 => SmsStatus::AddError,
			21 => SmsStatus::AuthError,
			29 => SmsStatus::VersionError,
			134 => SmsStatus::TrafficRestrictions,
			99 => SmsStatus::OtherError,
			_ => { SmsStatus::OtherError }
		}
	}

	///通过给定的账号密码。计算实际向客户发送的消息，也是用来进行校验密码是否正确。
	fn get_auth(&self, sp_id: &str, password: &str, timestamp: u32) -> [u8; 16] {
		let time_str = format!("{:010}", timestamp);

		//用于鉴别源地址。其值通过单向MD5 hash计算得出，表示如下：
		// AuthenticatorSource =
		// MD5（Source_Addr+7 字节的0 +shared secret+timestamp）
		// Shared secret 由中国移动与源地址实体事先商定，timestamp格式为：MMDDHHMMSS，即月日时分秒，10位。
		let mut src = BytesMut::with_capacity(sp_id.len() + 7 + password.len() + 10);

		src.put_slice(sp_id.as_bytes());
		//加起来7位
		src.put_u32(0);
		src.put_u16(0);
		src.put_u8(0);

		src.put_slice(password.as_bytes());
		src.put_slice(time_str.as_bytes());

		let (src, _) = src.split_at(src.len());

		md5::compute(src).0
	}

	///生成登录操作的消息
	fn encode_connect(&self, json: &mut JsonValue) -> Result<BytesMut, io::Error> {
		let client_id = match json[LOGIN_NAME].as_str() {
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

		let version = match json[VERSION].as_u8() {
			Some(v) => v,
			None => {
				log::error!("没有version.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有version"));
			}
		};

		let time = get_time();

		//固定这个大小。
		let mut dst = BytesMut::with_capacity(42);
		dst.put_u32(42);
		dst.put_u32(self.get_type_id(Connect));
		dst.put_u32(get_sequence_id(1));

		fill_bytes_zero(&mut dst, client_id, 8);  //ClientID	8
		dst.extend_from_slice(self.get_auth(client_id, password, time).as_ref());
		dst.put_u8(2);//"LoginMode"	1 2:收发消息
		dst.put_u32(time); //TimeStamp"	4	Integer
		dst.put_u8(version); //ClientVersion  1  Integer

		Ok(dst)
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
		let msg_id = self.create_msg_id(dest_ids.len() as u32);
		json[MSG_ID] = smgp_msg_id_buf_to_str(&msg_id).into();

		let mut buf = BytesMut::with_capacity(26);
		buf.put_u32(26);
		buf.put_u32(self.get_type_id(SubmitResp));
		buf.put_u32(seq_id);
		buf.put(msg_id);
		buf.put_u32(submit_status);

		Some(buf)
	}

	fn encode_report(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => smgp_msg_id_str_to_buf(v)
		};

		let receive_time = match json[SMGP_RECEIVE_TIME].as_str() {
			None => {
				log::error!("没有receive_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有receive_time"));
			}
			Some(v) => v
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
		let mut dst = BytesMut::with_capacity(183);

		dst.put_u32(183);
		dst.put_u32(self.get_type_id(MsgType::Deliver));
		let mut seq_ids = Vec::with_capacity(1);
		let seq_id = get_sequence_id(1);
		seq_ids.push(seq_id);
		dst.put_u32(seq_id);

		dst.put(msg_id.clone());
		dst.put_u8(1); //IsReport
		dst.put_u8(0); //MsgFormat
		fill_bytes_zero(&mut dst, receive_time, 14);//receive_time 14
		fill_bytes_zero(&mut dst, src_id, 21);//src_id 21
		fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21

		// id:XXXXXXXXXX sub:000 dlvrd:000 Submit_Date:0901151559 Done_Date:0901151559 Stat:DELIVRD err:000 text:
		dst.put_u8(102); //Msg_Length 状态报告长度固定
		dst.extend_from_slice("id:".as_bytes()); //id:
		dst.put(msg_id);// 10位的msg_id
		dst.extend_from_slice(" sub:001  dlvrd:000 Submit_Date:".as_bytes()); //这些是固定格式
		fill_bytes_zero(&mut dst, submit_time, 10); //submit_time 10
		dst.extend_from_slice(" Done_Date:".as_bytes());
		fill_bytes_zero(&mut dst, done_time, 10); //done_time 10
		dst.extend_from_slice("  Stat:".as_bytes());
		fill_bytes_zero(&mut dst, stat, 7); //Stat 7
		dst.extend_from_slice("   err:000 text:".as_bytes());

		json[SEQ_IDS] = seq_ids.into();

		Ok(dst)
	}

	fn encode_deliver(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let _msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => smgp_msg_id_str_to_buf(v)
		};

		let receive_time = match json[SMGP_RECEIVE_TIME].as_str() {
			None => {
				log::error!("没有receive_time.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有receive_time"));
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

		let msg_fmt = json[MSG_FMT].as_u8().unwrap_or(15);
		//编码以后的消息内容
		let msg_content_code = match encode_msg_content(msg_fmt,msg_content) {
			Ok(v) => v,
			Err(e) => {
				log::error!("字符串内容解码出现错误..json:{}.e:{}", json, e);
				return Err(io::Error::new(io::ErrorKind::Other, "字符串内容解码出现错误"));
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
		let mut tlv_len: usize = 0;
		let mut tlvs = Vec::new();

		//整个消息的长度
		let sms_len = if msg_content_len <= 140 {
			1 as usize
		} else {
			msg_content_head_len = 6;
			one_content_len = one_content_len - 6;
			msg_content_seq_id = get_sequence_id(1) as u8;

			//增加一个可选项.
			tlv_len += 5;
			tlvs.push(SmgpTLV::TPUdhi(1));

			((msg_content_len as f32) / one_content_len as f32).ceil() as usize
		};

		//长短信的话,一次性生成多条记录
		//77是除开内容之后所有长度加在一起
		let total_len = sms_len * (89 + msg_content_head_len + tlv_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((89 + msg_content_head_len + this_msg_content.len() + tlv_len) as u32);
			dst.put_u32(self.get_type_id(MsgType::Deliver));
			let seq_id = get_sequence_id(1);
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);
			seq_ids.push(seq_id);

			dst.put(self.create_msg_id(1));

			dst.put_u8(0); //IsReport
			dst.put_u8(msg_fmt); //MsgFormat
			fill_bytes_zero(&mut dst, receive_time, 14);//receive_time 14
			fill_bytes_zero(&mut dst, src_id, 21);//src_id 21
			fill_bytes_zero(&mut dst, dest_id, 21);//dest_id 21
			dst.put_u8((this_msg_content.len() + msg_content_head_len) as u8); //Msg_Length
			if msg_content_head_len > 0 {
				dst.put_u8(5);
				dst.put_u8(0);
				dst.put_u8(03);
				dst.put_u8(msg_content_seq_id);
				dst.put_u8(sms_len as u8);
				dst.put_u8((i + 1) as u8);
			}
			dst.extend_from_slice(&this_msg_content[..]); //Msg_Content
			dst.extend_from_slice(&FILL_ZERO[0..8]); //Reserve

			if !tlvs.is_empty() { self.coder_tlv(&mut dst, &tlvs,i + 1); }
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

		//进行一下检测
		if !json[MSG_IDS].is_array() {
			log::error!("没有msg_ids.退出..json:{}", json);
			return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_ids"));
		};

		let msg_fmt = json[MSG_FMT].as_u8().unwrap_or(15);

		//编码以后的消息内容
		let msg_content_code = match encode_msg_content(msg_fmt,msg_content) {
			Ok(v) => v,
			Err(e) => {
				log::error!("字符串内容解码出现错误..json:{}.e:{}", json, e);
				return Err(io::Error::new(io::ErrorKind::Other, "字符串内容解码出现错误"));
			}
		};

		let at_time = match json[AT_TIME].as_str() {
			Some(v) => v,
			None => "",
		};

		let service_id = match json[SERVICE_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有service_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有service_id"));
			}
		};

		let valid_time = match json[VALID_TIME].as_str() {
			Some(v) => v,
			None => "",
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
		let mut tlv_len: usize = 0;
		let mut tlvs = Vec::new();

		//整个消息的长度
		let sms_len = if msg_content_len <= 140 {
			1 as usize
		} else {
			msg_content_head_len = 6;
			one_content_len = one_content_len - 6;
			msg_content_seq_id = get_sequence_id(1) as u8;

			//增加可选项.
			tlv_len += 15;
			tlvs.push(SmgpTLV::TPUdhi(1));
			tlvs.push(SmgpTLV::PkTotal(((msg_content_len as f32) / one_content_len as f32).ceil() as u8));
			tlvs.push(SmgpTLV::PkNumber);

			((msg_content_len as f32) / one_content_len as f32).ceil() as usize
		};

		//长短信的话,一次性生成多条记录
		//126是除开内容\发送号码之后所有长度加在一起 6是长短信消息头长度
		//还需要增加相应的tlv的长度.这里只有一个.
		let total_len = sms_len * (126 + dest_ids.len() * 21 + msg_content_head_len + tlv_len) + msg_content_len;

		// 126 + msg_content_len + dest_ids.len() * 21;
		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((126 + dest_ids.len() * 21 + msg_content_head_len + this_msg_content.len() + tlv_len) as u32);
			dst.put_u32(self.get_type_id(MsgType::Submit));
			let seq_id = get_sequence_id(dest_ids.len() as u32);
			seq_ids.push(seq_id);
			dst.put_u32(seq_id);

			dst.put_u8(6); //MsgType
			dst.put_u8(1); //NeedReport
			dst.put_u8(2); //Priority
			fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
			dst.extend_from_slice("00".as_bytes());//FeeType
			dst.extend_from_slice("000000".as_bytes());//FeeCode
			dst.extend_from_slice("000000".as_bytes());//FixedFee
			dst.put_u8(msg_fmt); //MsgFormat
			fill_bytes_zero(&mut dst, valid_time, 17);  //valid_time
			fill_bytes_zero(&mut dst, at_time, 17);  //at_time
			fill_bytes_zero(&mut dst, src_id, 21);  //src_id
			dst.extend_from_slice(&FILL_ZERO[0..21]);//ChargeTermID
			dst.put_u8(dest_ids.len() as u8); //DestTermIDCount
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

			if !tlvs.is_empty() { self.coder_tlv(&mut dst, &tlvs,i + 1); }
		}

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}

	fn decode_submit_or_deliver_resp(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		let msg_id = smgp_msg_id_buf_to_str(&buf.split_to(10));
		json[MSG_ID] = msg_id.into();

		let result = buf.get_u32();//result 4
		json[RESULT] = result.into();
		
		if self.is_speed_limit(result) {
			json[SPEED_LIMIT] = true.into();
		};

		Ok(json)
	}

	fn is_speed_limit(&self, code: u32) -> bool{
		return code == 75
	}

	fn decode_deliver(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		let msg_id = smgp_msg_id_buf_to_str(&buf.split_to(10));
		json[MSG_ID] = msg_id.into();
		let is_report = buf.get_u8(); //Registered_Delivery	1
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
		json[SMGP_RECEIVE_TIME] = load_utf8_string(buf, 14).into(); //RecvTime	14
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		json[DEST_ID] = load_utf8_string(buf, 21).into(); //dest_id 21

		let msg_content_len = buf.get_u8(); //Msg_Length	1
		let mut content_buf = buf.split_to(msg_content_len as usize);// MsgContent
		buf.advance(8); //Reserve	8

		let mut need_tlvs = vec![SmgpTLV::TPUdhi(0)];
		self.decode_tlvs(buf, &mut need_tlvs);
		let is_long_sms = if let SmgpTLV::TPUdhi(v) = need_tlvs[0] {
			v == 1
		} else {
			false
		};

		if is_report == 0 {
			json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
			decode_msg_content(&mut content_buf, msg_fmt, msg_content_len, &mut json, is_long_sms)?;
		} else {
				// id:XXXXXXXXXX sub:000 dlvrd:000 Submit_Date:0901151559 Done_Date:0901151559 Stat:DELIVRD err:000 text:
			json[IS_REPORT] = true.into(); //状态报告增加.

			content_buf.advance(3);//名字："id:"

			let msg_id = smgp_msg_id_buf_to_str(&content_buf.split_to(10));//"id的实际值"
			json[PASSAGE_MSG_ID] = msg_id.into();

			content_buf.advance(18); //名字：“ sub:001 dlvrd:001”

			content_buf.advance(13); //“ Submit_Date:”
			json[SUBMIT_TIME] = load_utf8_string(&mut content_buf, 10).into(); // Submit_Date
			
			content_buf.advance(11); //“ Done_Date:”
			json[DONE_TIME] = load_utf8_string(&mut content_buf, 10).into(); // Done_Date
			
			content_buf.advance(6); //“ Stat:”
			json[STATE] = load_utf8_string(&mut content_buf, 7).into(); // Stat

			content_buf.advance(5); //“ Err:”
			json[ERROR_CODE] = load_utf8_string(&mut content_buf, 3).into(); // ERR


			//是状态报告.修改一下返回的类型.
			json[MSG_TYPE_U32] = self.get_type_id(MsgType::Report).into();
		}

		Ok(json)
	}

	fn decode_submit(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		buf.advance(27); //MsgType 1 NeedReport 1 Priority 1 ServiceID	10 FeeType	2 FeeCode	6 FixedFee	6
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1
		json[VALID_TIME] = load_utf8_string(buf, 17).into(); //valid_time 17
		json[AT_TIME] = load_utf8_string(buf, 17).into(); //at_time 17
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		buf.advance(21);//ChargeTermID	21
		let dest_len = buf.get_u8(); //DestUsr_tl 1
		//dest_id 32
		let mut dest_ids: Vec<String> = Vec::new();
		for _i in 0..dest_len {
			dest_ids.push(load_utf8_string(buf, 21));
		}

		let msg_content_len = buf.get_u8() as usize; //Msg_Length	1
		let mut content_buf = if msg_content_len < buf.len() {
			buf.split_to(msg_content_len)
		} else {
			log::warn!("消息结构出错.没有足够的长度读消息.");
			return Err(io::Error::new(io::ErrorKind::Other, "没有足够的长度读消息."));
		};
		buf.advance(8); //Reserve 8

		let mut need_tlvs = vec![SmgpTLV::TPUdhi(0)];
		self.decode_tlvs(buf, &mut need_tlvs);
		let is_long_sms = if let SmgpTLV::TPUdhi(v) = need_tlvs[0] {
			v == 1
		} else {
			false
		};

		decode_msg_content(&mut content_buf, msg_fmt, msg_content_len as u8, &mut json, is_long_sms)?;
		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		json[LOGIN_NAME] = load_utf8_string(buf, 8).into();
		// json[AUTHENTICATOR] = copy_to_bytes(buf, 16).as_ref().into();
		//现在取2次,相当于只保存8位.
		json[AUTHENTICATOR] = buf.get_u64().into();
		json[AUTHENTICATOR] = buf.get_u64().into();
		buf.advance(1); // LoginMode *
		json[TIMESTAMP] = buf.get_u32().into();
		json[VERSION] = (buf.get_u8() as u32).into();


		Ok(json)
	}
}

impl Clone for Smgp30 {
	fn clone(&self) -> Self {
		Smgp30::new()
	}
}

impl Smgp30 {
	pub fn new() -> Self {
		Smgp30 {
			version: 0x30,
			length_codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}

	//根据跳过数量创建msgId。len:当前创建完成后，步进多少的值
	//必然返回10位长度。
	fn create_msg_id(&self, len: u32) -> BytesMut {
		let mut buf = BytesMut::with_capacity(10);
		unsafe{
			buf.set_len(buf.capacity());
		}

		let ismg = *ISMG_ID;
		for i in 0..3 {
			let d = ((ismg >> ((i * 2 + 1) * 4) & 0xF) << 4) | ismg >> (i * 2 * 4) & 0xF ;
			buf[2 - i] = d as u8;
		}

		let mut time = get_time() / 100; //去掉秒
		for i in 0..4 {
			let d = ((time / 10) % 10) << 4 | time % 10;
			time  /= 100;
			buf[6 - i] = d as u8;
		}

		let mut seq = get_sequence_id(len) % 1000000;
		for i in 0..3 {
			let d = ((seq / 10) % 10) << 4 | seq % 10;
			seq  /= 100;
			buf[9 - i] = d as u8;
		}

		buf
	}

	fn coder_tlv(&self, buf: &mut BytesMut, tlvs: &Vec<SmgpTLV>,sms_len: usize) {
		for tlv in tlvs.iter() {
			buf.put_u16(tlv.get_u16());
			match tlv {
				SmgpTLV::TPPid(v) |
				SmgpTLV::TPUdhi(v) |
				SmgpTLV::ChargeUserType(v) |
				SmgpTLV::ChargeTermType(v) |
				SmgpTLV::DestTermType(v) |
				SmgpTLV::PkTotal(v) |
				SmgpTLV::SubmitMsgType(v) |
				SmgpTLV::SPDealReslt(v) |
				SmgpTLV::SrcTermType(v) |
				SmgpTLV::NodesCount(v) |
				SmgpTLV::SrcType(v) => {
					buf.put_u16(1);
					buf.put_u8(*v);
				}
				SmgpTLV::PkNumber => {
					buf.put_u16(1);
					buf.put_u8(sms_len as u8);
				}
				SmgpTLV::DestTermPseudo(b) |
				SmgpTLV::MsgSrc(b) |
				SmgpTLV::MServiceID(b) |
				SmgpTLV::SrcTermPseudo(b) |
				SmgpTLV::LinkID(b) |
				SmgpTLV::ChargeTermPseudo(b) => {
					buf.put_u16(b.len() as u16);
					buf.extend_from_slice(b.as_ref());
				}
			}
		}
	}

	fn decode_tlvs(&self, buf: &mut BytesMut, tlvs: &mut Vec<SmgpTLV>) {
		//最少读到5位才够
		while buf.len() >= 5 {
			let key = buf.get_u16();
			let length = buf.get_u16();
			if buf.len() < length as usize {
				log::error!("解码tlv失败..没有足够的长度.");
				return;
			}

			if let Some(tlv) = tlvs.iter_mut().find(|item| item.get_u16() == key) {
				match tlv {
					SmgpTLV::TPPid(_) |
					SmgpTLV::TPUdhi(_) |
					SmgpTLV::ChargeUserType(_) |
					SmgpTLV::ChargeTermType(_) |
					SmgpTLV::DestTermType(_) |
					SmgpTLV::PkTotal(_) |
					SmgpTLV::PkNumber |
					SmgpTLV::SubmitMsgType(_) |
					SmgpTLV::SPDealReslt(_) |
					SmgpTLV::SrcTermType(_) |
					SmgpTLV::NodesCount(_) |
					SmgpTLV::SrcType(_) => {
						*tlv = SmgpTLV::from_u8(key, buf.get_u8());
					}
					SmgpTLV::DestTermPseudo(_) |
					SmgpTLV::MsgSrc(_) |
					SmgpTLV::MServiceID(_) |
					SmgpTLV::SrcTermPseudo(_) |
					SmgpTLV::LinkID(_) |
					SmgpTLV::ChargeTermPseudo(_) => {
						*tlv = SmgpTLV::from_slice(key, buf.split_to(length as usize));
					}
				}
			}
		}
	}
}

enum SmgpTLV {
	TPPid(u8),
	TPUdhi(u8),
	ChargeUserType(u8),
	LinkID(BytesMut),
	ChargeTermType(u8),
	ChargeTermPseudo(BytesMut),
	DestTermType(u8),
	DestTermPseudo(BytesMut),
	PkTotal(u8),
	PkNumber,
	SubmitMsgType(u8),
	SPDealReslt(u8),
	SrcTermType(u8),
	SrcTermPseudo(BytesMut),
	NodesCount(u8),
	MsgSrc(BytesMut),
	SrcType(u8),
	MServiceID(BytesMut),
}

impl SmgpTLV {
	fn get_u16(&self) -> u16 {
		match self {
			SmgpTLV::TPPid(_) => 0x0001,
			SmgpTLV::TPUdhi(_) => 0x0002,
			SmgpTLV::ChargeUserType(_) => 0x0004,
			SmgpTLV::LinkID(_) => 0x0003,
			SmgpTLV::ChargeTermType(_) => 0x0005,
			SmgpTLV::ChargeTermPseudo(_) => 0x0006,
			SmgpTLV::DestTermType(_) => 0x0007,
			SmgpTLV::DestTermPseudo(_) => 0x0008,
			SmgpTLV::PkTotal(_) => 0x0009,
			SmgpTLV::PkNumber => 0x000A,
			SmgpTLV::SubmitMsgType(_) => 0x000B,
			SmgpTLV::SPDealReslt(_) => 0x000C,
			SmgpTLV::SrcTermType(_) => 0x000D,
			SmgpTLV::SrcTermPseudo(_) => 0x000E,
			SmgpTLV::NodesCount(_) => 0x000F,
			SmgpTLV::MsgSrc(_) => 0x0010,
			SmgpTLV::SrcType(_) => 0x0011,
			SmgpTLV::MServiceID(_) => 0x0012,
		}
	}

	fn from_u8(id: u16, v: u8) -> Self {
		match id {
			0x0001 => SmgpTLV::TPPid(v),
			0x0002 => SmgpTLV::TPUdhi(v),
			0x0004 => SmgpTLV::ChargeUserType(v),
			0x0005 => SmgpTLV::ChargeTermType(v),
			0x0007 => SmgpTLV::DestTermType(v),
			0x0009 => SmgpTLV::PkTotal(v),
			0x000A => SmgpTLV::PkNumber,
			0x000B => SmgpTLV::SubmitMsgType(v),
			0x000C => SmgpTLV::SPDealReslt(v),
			0x000D => SmgpTLV::SrcTermType(v),
			0x000F => SmgpTLV::NodesCount(v),
			0x0011 => SmgpTLV::SrcType(v),
			_ => { SmgpTLV::SrcType(v) }
		}
	}

	fn from_slice(id: u16, v: BytesMut) -> Self {
		match id {
			0x0003 => SmgpTLV::LinkID(v),
			0x0006 => SmgpTLV::ChargeTermPseudo(v),
			0x0008 => SmgpTLV::DestTermPseudo(v),
			0x000E => SmgpTLV::SrcTermPseudo(v),
			0x0010 => SmgpTLV::MsgSrc(v),
			0x0012 => SmgpTLV::MServiceID(v),
			_ => { SmgpTLV::MServiceID(v) }
		}
	}
}
