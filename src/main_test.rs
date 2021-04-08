#![allow(unused)]

use sms_gate::protocol::{Cmpp48, ProtocolImpl, SmsStatus};
use tokio_util::codec::Decoder;
use json::JsonValue;
use std::io::Error;

///这里现在启动解码器的协议编码解码部分
fn main() {
	//设置日志启动
	simple_logger::SimpleLogger::new().init().unwrap();
	report();
}

fn report() {
	let mut cmpp = Cmpp48::new();


	let json = json::object! {
		msg_type:0x01000005,
		msg_id:123,
		service_id: "123456",
		tp_udhi:0,
		stat:"DELIVRD",
		submit_time:"2104031212",
		done_time:"2104031212",
		src_id:"18179156296",
		dest_id: "1068888888",
	};


	let (mut bytes, seq_ids) = cmpp.encode_from_entity_message(&json).unwrap();

	log::info!("打印生成的数据:{:#X},长度:{}.seq_ids:{:?}", bytes, bytes.len(), seq_ids);

	while bytes.len() > 0 {
		let mut json = match cmpp.decode(&mut bytes) {
			Ok(Some(v)) => {
				log::debug!("json:{}", v);
				v
			}
			Ok(None) => {
				log::info!("这里是收到空了.");
				return;
			}
			Err(e) => {
				log::error!("这里是收到错误...err:{}", e);
				return;
			}
		};

		let deliver_resp = cmpp.encode_receipt(SmsStatus::Success(()), &mut json);
		let mut bytes = deliver_resp.unwrap();
		log::info!("打印生成的数据:{:#X},长度:{}", bytes, bytes.len());

		let mut json = cmpp.decode(&mut bytes).unwrap().unwrap();

		log::debug!("json:{}", json);
	}
}

fn deliver() {
	let mut cmpp = Cmpp48::new();


	let json = json::object! {
		msg_type:0x00000005,
		msg_content:"版权声明：本文内容由阿里云实名注册用户自发贡献，版权归原作者所有，阿里云开发者社区不拥有其著作权，亦不承担相应法律责任。具体规则请查看《阿里云开发者社区用户服务协议》和《阿里云开发者社区知识产权保护指引》。如果您发现本社区中有涉嫌抄袭的内容，填写侵权投诉表单进行举报，一经查实，本社区将立刻删除涉嫌侵权内容",
		msg_id:123,
		service_id: "123456",
		tp_udhi:0,
		src_id:"106234123",
		dest_id: "18179156296",
	};


	let (mut bytes, seq_ids) = cmpp.encode_from_entity_message(&json).unwrap();

	log::info!("打印生成的数据:{:#X},长度:{}.seq_ids:{:?}", bytes, bytes.len(), seq_ids);

	while bytes.len() > 0 {
		let mut json = match cmpp.decode(&mut bytes) {
			Ok(Some(v)) => {
				log::debug!("json:{}", v);
				v
			}
			Ok(None) => {
				log::info!("这里是收到空了.");
				return;
			}
			Err(e) => {
				log::error!("这里是收到错误...err:{}", e);
				return;
			}
		};


		let deliver_resp = cmpp.encode_receipt(SmsStatus::Success(()), &mut json);
		let mut bytes = deliver_resp.unwrap();
		log::info!("打印生成的数据:{:#X},长度:{}", bytes, bytes.len());

		let mut json = cmpp.decode(&mut bytes).unwrap().unwrap();

		log::debug!("json:{}", json);
	}
}

fn submit() {
	let mut cmpp = Cmpp48::new();

	let json = json::object! {
		msg_type:0x00000004,
		msg_content:"byte 1 : 05, 表示剩余协议头的长度 ",
		msg_id:1233211234567 as u64,
		service_id: "987654321",
		tp_udhi:0,
		sp_id:"453212",
		valid_time:"9380571930847192834701",
		at_time:"9380571930847192834701",
		src_id:"106234123",
		dest_ids: [
			"18179156296",
			"18179156299",
			"18179156234",
		]
	};

	let (mut bytes, seq_ids) = cmpp.encode_from_entity_message(&json).unwrap();

	log::info!("打印生成的数据:{:#X},长度:{}.seq_ids:{:?}", bytes, bytes.len(), seq_ids);

	let mut json = cmpp.decode(&mut bytes).unwrap().unwrap();

	log::debug!("json:{}", json);


	let submit_resp = cmpp.encode_receipt(SmsStatus::TrafficRestrictions(()), &mut json);

	let mut bytes = submit_resp.unwrap();
	log::info!("打印生成的数据:{:#X},长度:{}", bytes, bytes.len());

	let mut json = cmpp.decode(&mut bytes).unwrap().unwrap();

	log::debug!("json:{}", json);
}


// fn login_msg() {
// 	let mut cmpp = Cmpp48::new();
//
// 	let mut bytes = cmpp.encode_login_msg("123456","123","3.0").unwrap();
//
// 	log::info!("打印生成的数据:{:#X},长度:{}",bytes ,bytes.len());
//
//
// 	match cmpp.decode(&mut bytes){
// 		Ok(Some(v)) => log::info!("成功:{}",v),
// 		Err(e) => {
// 			log::error!("这里是解码错误?? err = {:?}", e);
// 		}
// 		Ok(None) => {
// 			log::warn!("没有得到数据.")
// 		}
// 	}
// }
