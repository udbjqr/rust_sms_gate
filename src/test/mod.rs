#![allow(unused)]


use std::{collections::HashMap, ops::Add};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use chrono::{Datelike, Local};
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
fn test_smgp_report(){
	let mut buf = BytesMut::with_capacity(20);

	buf.put_u64(0x000000031070CF5B);
	buf.put_u64(0x0570030707155557);
	buf.put_u64(0x6459010032303231);
	buf.put_u64(0x3037303731353535);
	buf.put_u64(0x3535383631383137);
	buf.put_u64(0x3931353632393600);
	buf.put_u64(0x0000000000000031);
	buf.put_u64(0x3036383330373433);
	buf.put_u64(0x3333313233343536);
	buf.put_u64(0x000000007A69643A);
	buf.put_u64(0x0570030707155557);
	buf.put_u64(0x6459735375623A30);
	buf.put_u64(0x303173446C767264);
	buf.put_u64(0x3A30303073537562);
	buf.put_u64(0x6D69745F44617465);
	buf.put_u64(0x3A32313037303731);
	buf.put_u64(0x35353573446F6E65);
	buf.put_u64(0x5F446174653A3231);
	buf.put_u64(0x3037303731353535);
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


#[test]
fn test_smgp_long(){
	let mut json = json::object! {tp_udhi:0,
		msg_fmt:8,
		src_id:"10683074333123456",
		msg_content:"【签名】准备发送一条长短信。应该是多条在一起的。我也不知道这会有多少条。反正应该是挺多的。这个是李端的手机？？？看到这条消息说明长短信收发成功。如果没有看到。说明今天要加晚班改代这是准备发送一条长短信。应该是多条在一起的。我也不知道这会有多少条。反正应该是挺多的。这个是李端的手机？？？看到这条消息说明长短信收发成功。如果没有看到。说明今天要加晚班改代码。。。。。。。。。。。。。。",
		at_time:"",
		msg_ids:["070716434445698324119","070716434445698324120"],
		msg_type:"Submit",
		seq_id:1918637221,
		id:11,
		valid_time:"",
		dest_ids:["17779522835"],
		manager_type:"send",
		entity_id:11,
		spId:"101016",
		serviceId:"36201350050006",
		seq_ids:[2361810514u32,2361810515u32],
		wait_receipt:true,
		receive_time:1625647574,
		need_re_send:true,
		account_msg_id:"070716434445698324119"
	};

	let c = Smgp30::new(); 

	let buf = c.encode_submit(&mut json).unwrap();



	println!("{:X}",buf);
}


