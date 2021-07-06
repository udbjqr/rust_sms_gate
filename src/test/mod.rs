#![allow(unused)]


use std::{collections::HashMap, ops::Add};

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