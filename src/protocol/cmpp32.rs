use tokio_util::codec::{LengthDelimitedCodec, Decoder};
use crate::protocol::implements::{ProtocolImpl, create_cmpp_msg_id, fill_bytes_zero, load_utf8_string, decode_msg_content, cmpp_msg_id_u64_to_str, cmpp_msg_id_str_to_u64};
use json::JsonValue;
use bytes::{BytesMut, BufMut, Buf};
use crate::protocol::names::{AUTHENTICATOR, SEQ_ID, VERSION, MSG_ID, SERVICE_ID, STATE, SUBMIT_TIME, DONE_TIME, SMSC_SEQUENCE, SRC_ID, DEST_ID, SEQ_IDS, MSG_CONTENT, SP_ID, VALID_TIME, AT_TIME, DEST_IDS, MSG_TYPE_U32, RESULT, MSG_FMT, IS_REPORT, MSG_IDS};
use crate::protocol::{MsgType, SmsStatus};
use std::io::Error;
use crate::protocol::msg_type::MsgType::SubmitResp;
use tokio::io;
use crate::global::get_sequence_id;
use encoding::all::UTF_16BE;
use encoding::{Encoding, EncoderTrap};
use crate::global::FILL_ZERO;

///CMPP协议2.0的处理
#[derive(Debug, Default)]
pub struct Cmpp32 {
	version: u32,
	codec: LengthDelimitedCodec,
}

impl ProtocolImpl for Cmpp32 {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>> {
		self.codec.decode(buf)
	}

	///根据对方给的请求,处理以后的编码消息
	fn encode_connect_rep(&self, status: SmsStatus, json: &mut JsonValue) -> Option<BytesMut> {
		let mut dst = BytesMut::with_capacity(30);
		dst.put_u32(30);
		dst.put_u32(self.get_type_id(MsgType::ConnectResp));

		dst.put_u32(json[SEQ_ID].as_u32().unwrap());
		dst.put_u8(self.get_status_id(&status) as u8);

		let str = "0000000000000000".as_bytes();

		dst.extend_from_slice(&str[0..16]);
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

		let mut buf = BytesMut::with_capacity(21);
		buf.put_u32(21);
		buf.put_u32(self.get_type_id(SubmitResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u8(submit_status as u8);

		json[MSG_ID] = cmpp_msg_id_u64_to_str(msg_id).into();

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

		let mut buf = BytesMut::with_capacity(21);

		buf.put_u32(21);
		buf.put_u32(self.get_type_id(MsgType::DeliverResp));
		buf.put_u32(seq_id);
		buf.put_u64(msg_id);
		buf.put_u8(status as u8);

		Some(buf)
	}

	fn encode_report(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		//只检测一下.如果没有后续不处理.
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
		let mut dst = BytesMut::with_capacity(133);

		dst.put_u32(133);
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

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}

	fn encode_deliver(&self, json: &mut JsonValue) -> Result<BytesMut, Error> {
		//只检测一下.如果没有后续不处理.
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

			dst.put_u64(msg_id); //Msg_Id 8
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
			None => "",
		};

		let at_time = match json[AT_TIME].as_str() {
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
		let total_len = sms_len * (138 + dest_ids.len() * 21 + msg_content_head_len) + msg_content_len;

		let mut dst = BytesMut::with_capacity(total_len);
		let mut seq_ids = Vec::with_capacity(sms_len);

		for i in 0..sms_len {
			let this_msg_content = if i == sms_len - 1 {
				&msg_content_code[(i * one_content_len)..msg_content_code.len()]
			} else {
				&msg_content_code[(i * one_content_len)..((i + 1) * one_content_len)]
			};

			dst.put_u32((138 + dest_ids.len() * 21 + msg_content_head_len + this_msg_content.len()) as u32);
			dst.put_u32(self.get_type_id(MsgType::Submit));
			let seq_id = get_sequence_id(dest_ids.len() as u32);
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

		json[SEQ_IDS] = seq_ids.into();
		Ok(dst)
	}


	fn decode_submit_or_deliver_resp(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();

		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();
		json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into();//msg_id 8
		json[RESULT] = (buf.get_u8() as u32).into();//result 1

		Ok(json)
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
		json[SRC_ID] = load_utf8_string(buf, 21).into(); //src_id 21
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
			json[SRC_ID] = load_utf8_string(buf, 21).into(); // dest_terminal_id

			//是状态报告.修改一下返回的类型.
			json[MSG_TYPE_U32] = self.get_type_id(MsgType::Report).into();
		}

		Ok(json)
	}

	fn decode_submit(&self, buf: &mut BytesMut, seq: u32, tp: u32) -> Result<JsonValue, io::Error> {
		let mut json = JsonValue::new_object();
		json[MSG_TYPE_U32] = tp.into();
		json[SEQ_ID] = seq.into();

		json[MSG_ID] = cmpp_msg_id_u64_to_str(buf.get_u64()).into(); //msg_id 8
		buf.advance(4); //Pk_total 1 Pk_number 1 Registered_Delivery 1 Msg_level 1
		json[SERVICE_ID] = load_utf8_string(buf, 10).into(); //service_id 10
		buf.advance(23); //Fee_UserType 1  Fee_terminal_Id 21 TP_pId	1
		let tp_udhi = buf.get_u8(); //是否长短信
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
		let msg_content_len = buf.get_u8(); //Msg_Length	1
		decode_msg_content(buf, msg_fmt, msg_content_len, &mut json, tp_udhi != 0)?;

		Ok(json)
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
}
