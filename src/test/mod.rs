#![allow(unused)]


use std::{collections::HashMap, ops::Add};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use json::JsonValue;
use tokio::time::Instant;
use crate::{get_runtime, protocol::{cmpp32::Cmpp32, names::{DEST_ID, DEST_IDS, LONG_SMS_NOW_NUMBER, LONG_SMS_TOTAL, MSG_CONTENT, MSG_ID, MSG_IDS, SRC_ID}}};
use tokio::time;
use crate::protocol::Protocol::SMGP;
use crate::protocol::smgp::Smgp30;
use crate::protocol::ProtocolImpl;
use crate::protocol::implements::get_time;

mod entity;


#[test]
fn test_json2() {
	// time::Duration::from_millis();
	let d = get_runtime().spawn(async move {
		loop {
			let b = crate::global::get_sequence_id(1);
			tokio::select! {
				biased;
				_ = time::sleep(time::Duration::from_secs(2)) =>{
					println!("one");
				}
				_ = time::sleep(time::Duration::from_secs(1)) =>{
					println!("two");
				}
			}
		}
	});

	std::thread::sleep(time::Duration::from_secs(15));
}



#[test]
fn test_smgpconnect() {
	let c = Smgp30::new();
	let mut json = json::object! {
		loginName:"103996",
		password:"123456",
		protocolVersion: 48,
		msg_type: "Connect"
	};

	let e = c.encode_connect(&mut json).unwrap();

	println!("{:x}",e);
}

#[test]
fn test_test() {
	simple_logger::SimpleLogger::init(Default::default());

	use crate::protocol::implements::{ cmpp_msg_id_u64_to_str, cmpp_msg_id_str_to_u64};
	use bytes::{BytesMut, Buf, Bytes, BufMut};
	use json::JsonValue;
	use crate::protocol::names::{SEQ_ID, VERSION, MSG_ID, SERVICE_ID, STATE, SUBMIT_TIME, DONE_TIME, SMSC_SEQUENCE, SRC_ID, DEST_ID, SEQ_IDS, MSG_CONTENT, SP_ID, VALID_TIME, AT_TIME, DEST_IDS, MSG_TYPE_U32, RESULT, MSG_FMT, IS_REPORT, MSG_IDS};
	use crate::protocol::{MsgType, SmsStatus};

	let mut cmpp = Cmpp32::new();
	let mut json = json::object!{
		tp_udhi:0,
		msg_fmt:15,
		src_id:"17779522835",
		msg_content:"上行测试",
		passage_id:999,
		msg_type:"Deliver",
		id:20,
		dest_id:"1068220312345123",
		msg_id:"062323000003263749403",
		manager_type:"send",
		entity_id:20,
		spId:"103996",
		serviceId:"15664"
	};

	cmpp.encode_deliver(&mut json);
	println!("{:?}",json);

}

#[test]
fn test_map(){
	let mut long_sms_cache: HashMap<String, Vec<Option<String>>> = HashMap::new();

	test_map1(&mut long_sms_cache);
	test_map1(&mut long_sms_cache);
}


fn test_map1(long_sms_cache: &mut  HashMap<String, Vec<Option<String>>>){
	let vec = match long_sms_cache.get_mut("key") {
		None => {
			let mut v = vec![None; 2usize];

			v[0] = Some("None".to_owned());
			long_sms_cache.insert("key".to_owned(), v);

			long_sms_cache.get_mut("key").unwrap()
		}
		Some(vec) => {
			*vec = vec![None; 10 as usize];
			vec[0] = Some("Some".to_owned());

			vec
		}
	};

	println!("vec:{:?}",vec);
	println!("long_sms_cache:{:?}",long_sms_cache);
}

fn handle_long_sms(long_sms_cache: &mut HashMap<String, Vec<Option<JsonValue>>>, msg: JsonValue, total: u8) -> Option<JsonValue> {
	let key = String::from(msg[SRC_ID].as_str().unwrap_or("")).
		add(msg[DEST_IDS][0].as_str().unwrap_or("")).
		add(msg[DEST_ID].as_str().unwrap_or(""));

	let index = match msg[LONG_SMS_NOW_NUMBER].as_u8() {
		None => {
			log::error!("没有当前是长短信的第几个的信息。不能放入。msg:{}", msg);
			return None;
		}
		Some(index) => {
			if index > total || total < 2 {
				log::error!("当前索引超过最大长度,或者总长度小于2。不能放入。msg:{}", msg);
				return None;
			}

			(index - 1) as usize
		}
	};

	let vec = match long_sms_cache.get_mut(&key) {
		None => {
			let mut vec = vec![None; total as usize];

			vec[index] = Some(msg);
			long_sms_cache.insert(key, vec);

			return None;
		}
		Some(vec) => {
			vec[index] = Some(msg);

			vec
		}
	};

	//没满,退出
	if let Some(_) = vec.iter().find(|json| json.is_none()) {
		return None;
	}

	//满了的处理方式
	let mut msg_ids = Vec::with_capacity(total as usize);
	let mut msg_content = String::new();

	for item in vec.iter() {
		let item = item.as_ref().unwrap();
		let msg_id = match item[MSG_ID].as_str() {
			None => {
				log::error!("从数据里面没有找到.msg_id字段...{}", item);
				""
			}
			Some(msg_id) => msg_id
		};

		msg_ids.push(msg_id.to_owned());
		msg_content.push_str(item[MSG_CONTENT].as_str().unwrap_or(""));
	}

	let mut json = vec.remove(0).unwrap();

	json[MSG_CONTENT] = msg_content.as_str().into();
	json[MSG_IDS] = msg_ids.into();
	json.remove(MSG_ID);
	json.remove(LONG_SMS_TOTAL);
	json.remove(LONG_SMS_NOW_NUMBER);

	long_sms_cache.remove(&key);

	Some(json)
}



#[test]
fn test_smgp_submit_resp(){
	let mut buf = BytesMut::with_capacity(20);
	let mut buf2 = BytesMut::with_capacity(10);

	buf.put_u64(0x0570010706170734);
	buf.put_u16(0x5654);

  let s = format!("{:X}",buf);

	println!("{}",s);

	let mut is_even = false;
	let mut num = 0u8;
	for i in s.as_bytes().iter(){
		num += i - 0x30;

		if is_even {
			buf2.put_u8(num);
			num = 0;
			is_even = false;
		}else {
			num = (i - 0x30) << 4;
			is_even = true;
		}
	}

	println!("{:X}",buf2);
}


#[test]
fn test_byte_set(){
	let mut buf = BytesMut::with_capacity(10);
	unsafe{
		buf.set_len(buf.capacity());
	}

	let mut ismg: u32 = 0x543679;

	for i in 0..3 {
		let d = ((ismg >> ((i * 2 + 1) * 4) & 0xF) << 4) | ismg >> (i * 2 * 4) & 0xF ;
		buf[2 - i] = d as u8;
	}

	let mut time = get_time();
	time /= 100;
	for i in 0..4 {
		let d = ((time / 10) % 10) << 4 | time % 10;
		time  /= 100;
		buf[6 - i] = d as u8;
	}

	let mut seq = 2234342325u32 % 1000000;

	for i in 0..3 {
		let d = ((seq / 10) % 10) << 4 | seq % 10;
		seq  /= 100;
		buf[9 - i] = d as u8;
	}

	println!("{:X}",buf);
}



///读一个长度的utf-8 字符编码.确保是utf-8.并且当前值是有效值.
fn load_utf8_string(buf: &mut BytesMut, len: usize) -> String {
	unsafe {
		String::from_utf8_unchecked(Vec::from(copy_to_bytes(buf, len).as_ref())).trim_end_matches(char::from(0)).to_owned()
	}
}

///读一个长度字串.做一个异常的保护.
fn copy_to_bytes(buf: &mut BytesMut, len: usize) -> Bytes {
	if buf.len() >= len {
		buf.split_to(len).freeze()
	} else {
		log::error!("得到消息出现错误.消息没有足够长度.可用长度:{}.现有长度{}", buf.len(), len);
		Bytes::new()
	}
}

#[test]
fn test_smgp_report(){
	let mut buf = BytesMut::with_capacity(20);

	buf.put_u64(0x0570010706170734);
	buf.put_u64(0x0570010706170734);
	buf.put_u64(0x5654010032303231);
	buf.put_u64(0x3037303631373037);
	buf.put_u64(0x3231383631383137);
	buf.put_u64(0x3931353632393600);
	buf.put_u64(0x0000000000000031);
	buf.put_u64(0x3036383330373400);
	buf.put_u64(0x0000000000000000);
	buf.put_u64(0x000000007A69643A);
	buf.put_u64(0x0570010706170734);
	buf.put_u64(0x5654735375623A30);
	buf.put_u64(0x303173446C767264);
	buf.put_u64(0x3A30303073537562);
	buf.put_u64(0x6D69745F44617465);
	buf.put_u64(0x3A32313037303631);
	buf.put_u64(0x37303773446F6E65);
	buf.put_u64(0x5F446174653A3231);
	buf.put_u64(0x3037303631373037);
	buf.put_u64(0x73537461743A4445);
	buf.put_u64(0x4C49565244734572);
	buf.put_u64(0x723A303030735465);
	buf.put_u64(0x78743A3030374445);
	buf.put_u64(0x4C49565244000000);
	buf.put_u64(0x0000000000000000);
	buf.put_u32(0x00000000);
	buf.put_u16(0x0000);
	buf.put_u8(0x00);


	let tp = buf.get_u32();
	let seq = buf.get_u32();

	let mut msg_id_buf = buf.split_to(0);

	let c = Smgp30::new(); 

	let json = c.decode_deliver(&mut buf, seq, tp).unwrap();
	println!("{:?}",json);
}


