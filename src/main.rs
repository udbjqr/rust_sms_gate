use std::error::Error;

use tokio::sync::mpsc;

use sms_gate::{ClientEntity, ProtocolType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	simple_logger::SimpleLogger::new().init().expect("SimpleLogger 日志无法启动。");

	let addr = String::from("127.0.0.1:7890");
	let mut client = ClientEntity::new(addr, "103996".parse().unwrap(), String::from("123456"), 2, ProtocolType::CMPP);

	//这个是多个人用的
	let (send_to_server_tx, send_to_server_rx) = mpsc::channel::<String>(0xffffffffff);

	//关闭是每一个链接都需要的。可能并不在这里初始化
	let (close_channel_tx, close_channel_rx) = mpsc::channel::<String>(0xffffffffff);

	match client.start_work(close_channel_rx, send_to_server_tx.clone()).await {
		Ok(_) => {
			println!("成功")
		}
		Err(e) => {
			println!("打印一个错误 {}", e);
		}
	}

	println!("后面将这个对象给kafka的发送者:{:?}", send_to_server_rx);

	std::thread::park();
	Ok(())
}


// #[tokio::main]
// async fn main() {
// 	simple_logger::SimpleLogger::new().init().expect("SimpleLogger 日志无法启动。");
//
//
// 	let (tx, mut rx) = mpsc::channel::<String>(100);
//
// 	let mut kafka = KafkaMessageConsumer::new(
// 		String::from("192.168.101.99:9092"),
// 		String::from("test"),
// 		String::from("top_test"),
// 	);
//
// 	kafka.start(tx.clone()).expect("出错了");
//
// 	get_runtime().spawn(async move {
// 		loop {
// 			match timeout(Duration::from_secs(3), rx.recv()).await {
// 				Ok(msg) => {
// 					if let Some(msg) = msg {
// 						println!("收到的内容:{}", msg);
// 					}
// 				}
// 				Err(_) => {
// 					println!("接收超时。");
// 					kafka.close();
// 					return;
// 				}
// 			}
// 		}
// 	});
//
// 	std::thread::park();
// }
