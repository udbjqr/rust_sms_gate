use std::collections::HashMap;
use std::fmt::{Debug};
use std::sync::Arc;

use json::JsonValue;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

pub use self::attach_custom::CustomEntity;
pub use self::services::ServersManager;
use crate::protocol::SmsStatus;

mod channel;
mod attach_custom;
mod attach_server;
mod services;
mod tcp_handle;



pub trait Entity: Send + Sync + Debug {
	///进行登录检验操作。如果成功。将通道附加至实体
	/// 返回值依次为:
	/// 登录状态,rx_limit,tx_limit,channel_rx,channel_tx
	fn login_attach(&self, json: JsonValue) -> (SmsStatus<JsonValue>, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>);

	fn send_message(&self, json: JsonValue);
	fn get_id(&self) -> u32;
	///当前通道的每秒读限制
	fn get_read_limit(&self) -> u32;
	///当前通道的每秒写限制
	fn get_write_limit(&self) -> u32;
}


static mut ENTITY_MAP: Option<Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>>> = None;

pub fn get_entity_map() -> Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>> {
	unsafe {
		ENTITY_MAP.get_or_insert_with(|| {
			Arc::new(RwLock::new(HashMap::new()))
		}).clone()
	}
}

pub async fn insert_entity(id: u32, item: Arc<dyn Entity>) {
	let map = get_entity_map();
	let mut write = map.write().await;

	write.insert(id, item);
}

pub async fn get_entity(id: u32) -> Option<Arc<dyn Entity>> {
	let en = get_entity_map();
	let read = en.read().await;
	return match read.get(&id) {
		Some(n) => Some(n.clone()),
		None => None
	};
}

///通道的代理类。放在实体里面了解通道相关状态使用
/// 并且保存与通道相对应的消息接收者
#[derive(Debug)]
pub struct ChannelProxy {
	///接收来自通道的消息
	from_channel: mpsc::Receiver<JsonValue>,
	to_channel: mpsc::Sender<JsonValue>,
	can_write: bool,
}
