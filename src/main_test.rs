use sms_gate::protocol::{Cmpp48, Protocol};

///这里现在启动解码器的协议编码解码部分
fn main() {
	//设置日志启动
	simple_logger::SimpleLogger::new().init().unwrap();

	submit();
}


fn submit() {
	let cmpp = Cmpp48::new();

	let json = json::object! {
		msg_type:0x00000004,
		msg_content:"1234567890-......",
		msg_id:1233211234567 as u64,
		service_id: "987654321",
		tp_udhi:0,
		sp_id:"453212",
		valid_time:"9380571930847192834701",
		at_time:"9380571930847192834701",
		src_id:"1062341234123423842423",
		dest_id:"192558744456"
	};

	let bytes = cmpp.encode_from_entity_message(&json).unwrap();

	log::info!("打印生成的数据:{:#X},长度:{}", bytes, bytes.len());
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
