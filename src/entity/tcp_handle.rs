use json::JsonValue;
use crate::global::{ message_sender};

pub async fn handle_rev_msg(msg: JsonValue) {
	log::debug!("开始进行接收消息的处理,msg:{}", msg);
	//TODO 进行收到消息的处理
	//TODO 直接向消息队列发送。不需要再拿对象
	//TODO 根据不同格式。发不同消息队列

	match msg["msg_type"].as_str() {
		Some("") => {
			let sender = message_sender();
			sender.send("topic","key",msg.as_str().unwrap()).await;
		}
		Some(_) => {
			log::error!("收到的msg_type未知。没有相应的处理方法。退出..msg:{}", msg);
			return;
		}
		None => {
			log::error!("收到消息处理出现异常。没有msg_type..msg:{}", msg);
			return;
		}
	}
}
