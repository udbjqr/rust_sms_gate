use std::fmt::Debug;

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc, RwLock};

use crate::protocol::SmsStatus;

pub use self::attach_custom::CustomEntity;
pub use self::entity_manager::EntityManager;
pub use self::services::ServersManager;
pub use self::entity_running::start_entity;

use std::sync::Arc;

#[macro_use]
mod channel;
mod attach_custom;
mod attach_server;
mod services;
mod tcp_handle;
mod entity_manager;
mod entity_running;

#[async_trait]
pub trait Entity: Send + Sync + Debug {
	///进行登录检验操作。如果成功。将通道附加至实体
	/// 返回值依次为:
	/// 登录状态,rx_limit,tx_limit,entity_to_channel_priority_tx,entity_to_channel_common_tx,channel_to_entity_tx
	async fn login_attach(&mut self, json: JsonValue) -> (SmsStatus<JsonValue>, u32, u32, Option<mpsc::Receiver<JsonValue>>,Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>);

	async fn send_message(&self, json: JsonValue);
	fn get_id(&self) -> u32;
	fn get_channels(&self) -> Arc<RwLock<Vec<ChannelStates>>>;
}

//
// static mut ENTITY_MAP: Option<Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>>> = None;
//
// pub fn get_entity_map() -> Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>> {
// 	unsafe {
// 		ENTITY_MAP.get_or_insert_with(|| {
// 			Arc::new(RwLock::new(HashMap::new()))
// 		}).clone()
// 	}
// }
//
// pub async fn insert_entity(id: u32, item: Arc<dyn Entity>) {
// 	let map = get_entity_map();
// 	let mut write = map.write().await;
//
// 	write.insert(id, item);
// }
//
// pub async fn get_entity(id: u32) -> Option<Arc<dyn Entity>> {
// 	let en = get_entity_map();
// 	let read = en.read().await;
//
// 	match read.get(&id) {
// 		Some(n) => Some(n.clone()),
// 		None => None
// 	}
// }

///通道的状态类。放在实体里面了解通道相关状态使用
/// 并且保存与通道相对应的消息接收者
#[derive(Debug)]
pub struct ChannelStates {
	is_active: bool,
	can_write: bool,
	///优先发送通道
	entity_to_channel_priority_tx: Option<mpsc::Sender<JsonValue>>,
	///普通发送通道
	entity_to_channel_common_tx: Option<mpsc::Sender<JsonValue>>,
}

impl ChannelStates {
	pub fn new() -> Self {
		ChannelStates{
			is_active: false,
			can_write: false,
			entity_to_channel_priority_tx: None,
			entity_to_channel_common_tx: None,
		}
	}
}
