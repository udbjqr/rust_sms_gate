#[allow(unused)]

use bytes::{BytesMut, BufMut, Buf};
use json::JsonValue;
use tokio::io;
use tokio::io::Error;
use tokio_util::codec::{Decoder, LengthDelimitedCodec};

use crate::protocol::{SmsStatus};
use crate::protocol::msg_type::MsgType;
use crate::protocol::implements::{ProtocolImpl, fill_bytes_zero, get_time, load_utf8_string, decode_msg_content};
use crate::protocol::names::{LOGIN_NAME, PASSWORD, MSG_TYPE_U32, SEQ_ID, RESULT, MSG_CONTENT, SERVICE_ID, SP_ID, SRC_ID, DEST_IDS, SEQ_IDS, MSG_FMT, DEST_ID, STATE, ERROR_CODE};
use crate::global::get_sequence_id;
use crate::protocol::msg_type::MsgType::{Connect, SubmitResp};
use crate::global::FILL_ZERO;

use super::implements::{encode_msg_content, sgip_msg_id_str_to_u64, sgip_msg_id_u64_to_str};
use super::names::{CAN_WRITE, MSG_ID, NODE_ID, PASSAGE_MSG_ID, SPEED_LIMIT, STATUS, VERSION};

///Sgip协议的处理
#[derive(Debug)]
pub struct Sgip {
	version: u32,
	length_codec: LengthDelimitedCodec,
}


impl ProtocolImpl for Sgip {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>> {
		self.length_codec.decode(buf)
	}
	
	fn is_speed_limit(&self, code: u32) -> bool{
		return code == 88
	}

	fn get_type_id(&self, t: MsgType) -> u32 {
		match t {
			MsgType::Submit => 0x00000003,
			MsgType::SubmitResp => 0x80000003,
			MsgType::Deliver => 0x00000004,
			MsgType::DeliverResp => 0x80000004,
			MsgType::Report => 0x00000005,
			MsgType::ReportResp => 0x80000005,
			MsgType::Connect => 0x00000001,
			MsgType::ConnectResp => 0x80000001,
			MsgType::Terminate => 0x00000002,
			MsgType::TerminateResp => 0x80000002,
			MsgType::Query => 0x00000007,
			MsgType::QueryResp => 0x80000007,
			MsgType::Cancel => 0x00000008,
			MsgType::CancelResp => 0x80000008,
			MsgType::ActiveTest => 0x00000009,
			MsgType::ActiveTestResp => 0x80000009,
			MsgType::UNKNOWN => 0,
			_ => 0
		}
	}

	fn get_type_enum(&self, v: u32) -> MsgType {
		match v {
			0x00000003 => MsgType::Submit,
			0x80000003 => MsgType::SubmitResp,
			0x00000004 => MsgType::Deliver,
			0x80000004 => MsgType::DeliverResp,
			0x00000005 => MsgType::Report,
			0x80000005 => MsgType::ReportResp,
			0x00000001 => MsgType::Connect,
			0x80000001 => MsgType::ConnectResp,
			0x00000002 => MsgType::Terminate,
			0x80000002 => MsgType::TerminateResp,
			0x00000007 => MsgType::Query,
			0x80000007 => MsgType::QueryResp,
			0x00000008 => MsgType::Cancel,
			0x80000008 => MsgType::CancelResp,
			0x00000009 => MsgType::ActiveTest,
			0x80000009 => MsgType::ActiveTestResp,
			_ => MsgType::UNKNOWN,
		}
	}

	fn get_status_id(&self, status: &SmsStatus) -> u32 {
		match status {
			SmsStatus::Success => 0,
			SmsStatus::MessageError => 7,
			SmsStatus::AddError => 2,
			SmsStatus::AuthError => 1,
			SmsStatus::VersionError => 4,
			SmsStatus::TrafficRestrictions => 101,
			SmsStatus::OtherError => 5,
			SmsStatus::UNKNOWN => 999,
		}
	}

	fn get_status_enum(&self, v: u32) -> SmsStatus {
		match v {
			0 => SmsStatus::Success,
			7 => SmsStatus::MessageError,
			2 => SmsStatus::AddError,
			1 => SmsStatus::AuthError,
			4 => SmsStatus::VersionError,
			101 => SmsStatus::TrafficRestrictions,
			5 => SmsStatus::OtherError,
			_ => { SmsStatus::OtherError }
		}
	}

	///通过给定的账号密码。计算实际向客户发送的消息，也是用来进行校验密码是否正确。
	fn get_auth(&self, _sp_id: &str, password: &str, _timestamp: u32) -> [u8; 16] {
		let mut input = [0u8; 16];

		let p = password.as_bytes();
		for i in 0..password.len().min(16) {
			input[i] = *p.get(i).unwrap();
		}

		input
	}

	///生成登录操作的消息
	fn encode_connect(&self, json: &mut JsonValue) -> Result<BytesMut, io::Error> {
		let login_name = match json[LOGIN_NAME].as_str() {
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

		let node_id = match json[SP_ID].as_str() {
			Some(v) => v.parse().unwrap_or(0),
			None => {
				log::error!("没有node_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有node_id"));
			}
		};

		//固定这个大小。
		let mut dst = BytesMut::with_capacity(61);
		dst.put_u32(61);
		dst.put_u32(self.get_type_id(Connect));
		dst.put_u32(node_id);
		dst.put_u32(get_time());
		dst.put_u32(get_sequence_id(1));
		dst.put_u8(1); //Login Type 1

		fill_bytes_zero(&mut dst, login_name, 16); //Login Name
		dst.extend_from_slice(&self.get_auth(login_name, password, get_time()));
		dst.extend_from_slice(&FILL_ZERO[0..8]);//Reserve

		Ok(dst)
	}

	///根据对方给的请求,处理以后的编码消息
	fn encode_connect_rep(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let node_id = match json[SP_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有node_id.退出..json:{}", json);
				return None;
			}
		};

		let mut dst = BytesMut::with_capacity(29);
		dst.put_u32(29);
		dst.put_u32(self.get_type_id(MsgType::ConnectResp));

		dst.put_u32(node_id);
		dst.put_u64(json[SEQ_ID].as_u64().unwrap_or(0));//seq_id

		dst.put_u8(self.get_status_id(&status) as u8); //Result
		dst.extend_from_slice(&FILL_ZERO[0..8]);//Reserve

		Some(dst)
	}


	fn encode_submit_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let msg_id = match json[MSG_ID].as_str() {
			Some(v) => sgip_msg_id_str_to_u64(v),
			None => {
				log::error!("没有MSG_ID.退出..json:{}", json);
				return None;
			}
		};

		let status = self.get_status_id(&status);
		let mut buf = BytesMut::with_capacity(29);
		buf.put_u32(29);
		buf.put_u32(self.get_type_id(SubmitResp));
		buf.put_u32(msg_id.0);
		buf.put_u64(msg_id.1);
		buf.put_u8(status as u8);
		buf.extend_from_slice(&FILL_ZERO[0..8]); //Reserve

		Some(buf)
	}

	fn encode_report_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let msg_id = match json[MSG_ID].as_str() {
			Some(v) => sgip_msg_id_str_to_u64(v),
			None => {
				log::error!("没有MSG_ID.退出..json:{}", json);
				return None;
			}
		};

		let status = self.get_status_id(&status);
		let mut buf = BytesMut::with_capacity(29);

		buf.put_u32(29);
		buf.put_u32(self.get_type_id(MsgType::ReportResp));
		buf.put_u32(msg_id.0);
		buf.put_u64(msg_id.1);
		buf.put_u8(status as u8);
		buf.extend_from_slice(&FILL_ZERO[0..8]); //Reserve

		Some(buf)
	}

	fn encode_deliver_resp(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let msg_id = match json[MSG_ID].as_str() {
			Some(v) => sgip_msg_id_str_to_u64(v),
			None => {
				log::error!("没有MSG_ID.退出..json:{}", json);
				return None;
			}
		};

		let status = self.get_status_id(&status);
		let mut buf = BytesMut::with_capacity(29);

		buf.put_u32(29);
		buf.put_u32(self.get_type_id(MsgType::DeliverResp));
		buf.put_u32(msg_id.0);
		buf.put_u64(msg_id.1);
		buf.put_u32(status);
		buf.extend_from_slice(&FILL_ZERO[0..8]); //Reserve

		Some(buf)
	}

	fn encode_terminate(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let node_id = match json[SP_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有node_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "node_id"));
			}
		};

		//状态报告长度固定
		let mut dst = BytesMut::with_capacity(20);

		dst.put_u32(20);
		dst.put_u32(self.get_type_id(MsgType::Terminate));
		dst.put_u32(node_id);
		let seq_id = (get_time() as u64) << 32 | get_sequence_id(1) as u64;
		let mut seq_ids = Vec::with_capacity(1);
		seq_ids.push(seq_id);
		dst.put_u64(seq_id);

		Ok(dst)
	}

	fn encode_report(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let msg_id = match json[MSG_ID].as_str() {
			None => {
				log::error!("没有msg_id字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有msg_id字串"));
			}
			Some(v) => sgip_msg_id_str_to_u64(v)
		};

		let pass_msg_id = match json[PASSAGE_MSG_ID].as_str() {
			None => {
				log::error!("没有PASSAGE_MSG_ID字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有PASSAGE_MSG_ID字串"));
			}
			Some(v) => sgip_msg_id_str_to_u64(v)
		};

		let stat = match json[STATE].as_u8() {
			Some(v) => v,
			None => {
				log::error!("没有state.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有state"));
			}
		};

		let err_code = match json[ERROR_CODE].as_u8() {
			Some(v) => v,
			None => {
				log::error!("没有error_code.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有error_code"));
			}
		};

		let src_id = match json[SRC_ID].as_str() {
			Some(v) => v,
			None => {
				log::error!("没有src_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有src_id"));
			}
		};

		//状态报告长度固定
		let mut dst = BytesMut::with_capacity(64);

		dst.put_u32(64);
		dst.put_u32(self.get_type_id(MsgType::Report));
		dst.put_u32(pass_msg_id.0);
		dst.put_u64(pass_msg_id.1);
		
		let mut seq_ids = Vec::with_capacity(1);
		seq_ids.push(msg_id.1);

		dst.put_u32(msg_id.0);
		dst.put_u64(msg_id.1); //这个对应之前客户给过来的submit的msgId
		dst.put_u8(0);//ReportType
		fill_bytes_zero(&mut dst, src_id, 21);//UserNumber 21
		dst.put_u8(stat);//State
		dst.put_u8(err_code);//err_code
		dst.extend_from_slice(&FILL_ZERO[0..8]); //Reserved

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}

	fn encode_active_test(&self, _json: &mut JsonValue) -> Result<BytesMut, Error> {
		//做一个无法发送的消息。做为通道活动请求。以下内容仅为符合通道数据。无其他意义
		let msg_content = "test";
		let msg_content_len = msg_content.len();
		let msg_format = 0;
		let service_id = "0";
		let corp_id = "00000";
		let node_id = 0;
		let src_id = "106877999833";
		let dest_ids = vec!["18877994444"];

		let total_len = 164 + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);

		dst.put_u32((164 + msg_content_len) as u32);
		dst.put_u32(self.get_type_id(MsgType::Submit));
		dst.put_u32(node_id);
		let seq_id = (get_time() as u64 ) << 32 | (get_sequence_id(dest_ids.len() as u32)) as u64;
		dst.put_u64(seq_id);


		fill_bytes_zero(&mut dst, src_id, 21);  //src_id:SPNumber
		dst.extend_from_slice(&FILL_ZERO[0..21]);//ChargeNumber
		dst.put_u8(dest_ids.len() as u8); //UserCount
		dest_ids.iter().for_each(|dest_id| {
			dst.extend_from_slice("86".as_bytes());
			fill_bytes_zero(&mut dst, dest_id, 19);
		});  //dest_id 19位.因为+86
		dst.extend_from_slice(corp_id[0..5].as_bytes()); //corp_id
		fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
		dst.put_u8(1); //FeeType
		dst.extend_from_slice("000001".as_ref()); //FeeCode
		dst.extend_from_slice("000000".as_ref()); //GivenValue
		dst.put_u8(0); //AgentFlag
		dst.put_u8(0); //MorelatetoMTFlag
		dst.put_u8(0); //Priority
		dst.extend_from_slice(&FILL_ZERO[0..16]); //ExpireTime
		dst.extend_from_slice(&FILL_ZERO[0..16]); //ScheduleTime
		dst.put_u8(1); //ReportFlag
		dst.put_u8(0); //TP_pId
		dst.put_u8(0); //tp_udhi
		dst.put_u8(msg_format); //Msg_Fmt
		dst.put_u8(0); //MessageType
		dst.put_u32(msg_content_len as u32); //Msg_Length
		dst.extend_from_slice(msg_content.as_bytes()); //Msg_Content
		dst.extend_from_slice(&FILL_ZERO[0..8]); //Reserve

		// let mut dst = BytesMut::with_capacity(total_len);
		Ok(dst)
	}

	fn encode_deliver(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		let node_id = match json[NODE_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有node_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "node_id"));
			}
		};

		let msg_content = match json[MSG_CONTENT].as_str() {
			None => {
				log::error!("没有内容字串.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有内容字串"));
			}
			Some(v) => v
		};

		let msg_fmt = json[MSG_FMT].as_u8().unwrap_or(8);
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
		//77是除开内容之后所有长度加在一起
		let total_len = sms_len * (77 + msg_content_head_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((77 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Deliver));
			dst.put_u32(node_id);
			let seq_id = (get_time() as u64) << 32 | get_sequence_id(1) as u64;
			seq_ids.push(seq_id);
			dst.put_u64(seq_id);

			fill_bytes_zero(&mut dst, src_id, 21);//src_id 21
			fill_bytes_zero(&mut dst, dest_id, 21);//src_id 21
			dst.put_u8(0); //TP_pid 1
			dst.put_u8(if sms_len == 1 { 0 } else { 1 }); //tp_udhi 1
			dst.put_u8(msg_fmt); //Msg_Fmt 1
			dst.put_u32((this_msg_content.len() + msg_content_head_len) as u32); //Msg_Length
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

		let msg_format = json[MSG_FMT].as_u8().unwrap_or(8);
		//编码以后的消息内容
		let msg_content_code = match encode_msg_content(msg_format,msg_content) {
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

		let corp_id:String = match json[SP_ID].as_str() {
			Some(v) => v.to_owned(),
			None => {
				log::error!("没有sp_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有sp_id"));
			}
		};

		let node_id = match json[NODE_ID].as_u32() {
			Some(v) => v,
			None => {
				log::error!("没有node_id.退出..json:{}", json);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有node_id"));
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
		//163是除开内容\发送号码之后所有长度加在一起 6是长短信消息头长度
		let total_len = sms_len * (143 + dest_ids.len() * 21 + msg_content_head_len) + msg_content_len;

		// 143 + msg_content_len + dest_ids.len() * 21;
		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((143 + dest_ids.len() * 21 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Submit));
			dst.put_u32(node_id);
			let seq_id = (get_time() as u64 ) << 32 | (get_sequence_id(dest_ids.len() as u32)) as u64;
			dst.put_u64(seq_id);

			seq_ids.push(seq_id);

			fill_bytes_zero(&mut dst, src_id, 21);  //src_id:SPNumber
			dst.extend_from_slice(&FILL_ZERO[0..21]);//ChargeNumber
			dst.put_u8(dest_ids.len() as u8); //UserCount
			dest_ids.iter().for_each(|dest_id| {
				dst.extend_from_slice("86".as_bytes());
				fill_bytes_zero(&mut dst, dest_id, 19);
			});  //dest_id 19位.因为+86
			dst.extend_from_slice(corp_id[0..5].as_bytes()); //corp_id
			fill_bytes_zero(&mut dst, service_id, 10);//Service_Id
			dst.put_u8(1); //FeeType
			dst.extend_from_slice("000001".as_ref()); //FeeCode
			dst.extend_from_slice("000000".as_ref()); //GivenValue
			dst.put_u8(0); //AgentFlag
			dst.put_u8(0); //MorelatetoMTFlag
			dst.put_u8(0); //Priority
			dst.extend_from_slice(&FILL_ZERO[0..16]); //ExpireTime
			dst.extend_from_slice(&FILL_ZERO[0..16]); //ScheduleTime
			dst.put_u8(1); //ReportFlag
			dst.put_u8(0); //TP_pId
			dst.put_u8(if sms_len == 1 { 0 } else { 1 }); //tp_udhi
			dst.put_u8(msg_format); //Msg_Fmt
			dst.put_u8(0); //MessageType
			dst.put_u32((this_msg_content.len() + msg_content_head_len) as u32); //Msg_Length
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
		}

		json[SEQ_IDS] = seq_ids.into();

		Ok(dst)
	}

	fn decode_connect_resp(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into(); 

		json[STATUS] = buf.get_u8().into();

		Ok(json)
	}

	fn decode_submit_or_deliver_resp(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();
		let result = buf.get_u8();//result 4
		json[RESULT] = result.into();
		
		if self.is_speed_limit(result as u32) {
			json[SPEED_LIMIT] = true.into();
		};
		Ok(json)
	}

	fn decode_nobody(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();

		Ok(json)
	}

	fn decode_report(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		json[SEQ_ID] = buf.get_u64().into();
		
		let node_id =  buf.get_u32().into();
		let seq_id =  buf.get_u64().into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();

		buf.advance(1); //ReportType
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		json[STATE]  = "DELIVRD".into();
		//State
		json[STATE] = match buf.get_u8(){
			0 => "DELIVRD".into(),
			1 => "ACCEPTD".into(),
			_ => "SGIPERR".into()
		};

		json[ERROR_CODE] = buf.get_u8().into(); //ErrorCode

		Ok(json)
	}

	fn decode_deliver(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();

		json[SRC_ID] = load_utf8_string(buf, 21).into(); // dest_terminal_id
		json[DEST_ID] = load_utf8_string(buf, 21).into(); //dest_id 21
		buf.advance(1); //TP_pid	1
		let tp_udhi = buf.get_u8(); //是否长短信
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //MessageCoding 1
		let msg_content_len = buf.get_u32(); //Msg_Length	4
		decode_msg_content(buf, msg_fmt, msg_content_len as u8, &mut json, tp_udhi != 0)?;

		Ok(json)
	}

	fn decode_report_resp(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into(); 

		json[RESULT] = buf.get_u8().into();

		Ok(json)
	}

	fn decode_submit(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();

		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
		buf.advance(21); //ChargeNumber
		let dest_len = buf.get_u8();//UserCount 1
		//dest_ids 21
		let mut dest_ids: Vec<String> = Vec::new();
		for _i in 0..dest_len {
			dest_ids.push(load_utf8_string(buf, 21));
		}
		json[DEST_IDS] = dest_ids.into();
		buf.advance(65); //CorpId 5  ServiceType 10  FeeType 1 FeeValue 6 GivenValue 6 AgentFlag 1  MorelatetoMTFlag 1  Priority 1 ExpireTime 16 ScheduleTime 16 ReportFlag 1 TP_pid 1
		let tp_udhi = buf.get_u8(); //是否长短信
		let msg_fmt = buf.get_u8();
		json[MSG_FMT] = msg_fmt.into(); //Msg_Fmt 1

		buf.advance(1); //MessageType 1
		//长短信的处理 tp_udhi != 0 说明是长短信
		let msg_content_len = buf.get_u32(); //Msg_Length	4
		decode_msg_content(buf, msg_fmt, msg_content_len as u8, &mut json, tp_udhi != 0)?;

		Ok(json)
	}

	///实际的解码连接消息
	fn decode_connect(&self, buf: &mut BytesMut, node_id: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();

		json[SP_ID] = node_id.into();
		let seq_id = buf.get_u64();
		json[SEQ_ID] = seq_id.into();
		json[MSG_ID] = sgip_msg_id_u64_to_str(node_id, seq_id).into();
		
		json[CAN_WRITE] = (buf.get_u8() == 1).into();//Login Type
		json[LOGIN_NAME] = load_utf8_string(buf, 16).into();//Login Name
		json[PASSWORD] = load_utf8_string(buf, 16).into(); //Login Passowrd
		json[VERSION] = 1.into();

		Ok(json)
	}
}

impl Clone for Sgip {
	fn clone(&self) -> Self {
		Sgip::new()
	}
}

impl Default for Sgip {
	fn default() -> Sgip {
		Sgip::new()
	}
}

impl Sgip {
	pub fn new() -> Self {
		Sgip {
			version: 0x0d,
			length_codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}
}	
