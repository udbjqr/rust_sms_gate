use std::fmt::Debug;

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc};

use crate::protocol::{SmsStatus};

pub use self::as_custom::CustomEntity;
pub use self::entity_manager::EntityManager;
pub use self::services::ServersManager;
pub use self::entity_running::start_entity;


#[macro_use]
pub mod channel;
mod as_custom;
mod as_server;
mod services;
mod entity_manager;
mod entity_running;

#[async_trait]
pub trait Entity: Send + Sync + Debug {
	/// 返回值依次为:
	/// id,登录状态,rx_limit,tx_limit,entity_to_channel_priority_rx,entity_to_channel_common_rx,channel_to_entity_tx
	async fn login_attach(&self) -> (usize, SmsStatus, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>);
	fn get_id(&self) -> u32;
	fn get_login_name(&self) -> &str;
	fn get_password(&self) -> &str;
	fn get_allow_ips(&self) -> &str;
	fn get_entity_type(&self) -> EntityType;
	fn is_server(&self) -> bool;
}

///通道的状态类。放在实体里面了解通道相关状态使用
/// 并且保存与通道相对应的消息接收者
#[derive(Debug, Clone)]
pub struct ChannelStates {
	id: usize,
	is_active: bool,
	can_write: bool,
	///优先发送通道
	entity_to_channel_priority_tx: mpsc::Sender<JsonValue>,
	///普通发送通道
	entity_to_channel_common_tx: mpsc::Sender<JsonValue>,
}


#[derive(Debug)]
pub enum EntityType {
	Custom,
	Server,
}
