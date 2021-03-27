use json::JsonValue;
use log::{debug, error, warn};
use rdkafka::Message;

use crate::entity::{ get_entity};
use crate::message_queue::QueueType;

///处理从消息队列过来的数据。
/// 这里应该已经是多协程处理了
pub async fn handle_queue_msg(msg: impl Message, queue_type: QueueType) {
	let key = match msg.key_view::<str>() {
		Some(Ok(key)) => {
			match key.parse::<u32>() {
				Ok(n) => n,
				Err(e) => {
					error!("转换key出错。key:{},e:{}", key, e);
					return;
				}
			}
		}
		_ => {
			error!("接收到的消息未指定key。。");
			return;
		}
	};

	let msg = match msg.payload_view::<str>() {
		Some(Ok(msg)) => {
			debug!("收到消息:key:{},msg:{}", key, msg);
			msg
		}
		Some(Err(e)) => {
			error!("获得数据出现错误:{}", e);
			return;
		}
		None => {
			warn!("解析结果为null");
			return;
		}
	};

	let json_msg = match json::parse(msg) {
		Ok(json_msg) => {
			json_msg
		}
		Err(e) => {
			error!("解码json格式出现错误:{}.文本:{}", e, msg);
			return;
		}
	};

	match queue_type {
		QueueType::SendMessage => { handle_send_to_entity(key, json_msg).await; }
		QueueType::CloseEntity => { error!("目前还未实现的方法。"); }
	};
}

async fn handle_send_to_entity(id: u32, json_msg: JsonValue) {
	let entity = match get_entity(id).await {
		Some(en) => en,
		_ => {
			error!("指定的Key不存在entity_map当中。。key:{}", id);
			return;
		}
	};

	entity.send_message(json_msg);
}
