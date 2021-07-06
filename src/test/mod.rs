#![allow(unused)]


use tokio::time::Instant;
use crate::{get_runtime, protocol::cmpp32::Cmpp32};
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

