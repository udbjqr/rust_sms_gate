use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::usize;

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc};

use crate::entity::{Entity, start_entity, EntityType};
use crate::get_runtime;
use crate::protocol::{SmsStatus};
use crate::global::{CHANNEL_BUFF_NUM, TEMP_SAVE, get_sequence_id};

///用来连接客户（下游）
#[derive(Debug)]
pub struct CustomEntity {
	id: u32,
	name: String,
	desc: String,
	login_name: String,
	password: String,
	allowed_addr: String,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: usize,
	now_channel_number: Arc<AtomicU8>,
	service_id: String,
	sp_id: String,
	config: JsonValue,
	accumulator: AtomicU64,
	send_to_manager_tx: mpsc::Sender<JsonValue>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
}

impl CustomEntity {
	pub fn new(id: u32,
	           name: String,
	           service_id: String,
	           sp_id: String,
	           desc: String,
	           login_name: String,
	           password: String,
	           allowed_addr: String,
	           read_limit: u32,
	           write_limit: u32,
	           max_channel_number: usize,
	           config: JsonValue,
	           send_to_manager_tx: mpsc::Sender<JsonValue>
	) -> Self {
		CustomEntity {
			id,
			name,
			service_id,
			sp_id,
			desc,
			login_name,
			password,
			allowed_addr,
			read_limit,
			write_limit,
			max_channel_number,
			now_channel_number: Arc::new(AtomicU8::new(0)),
			config,
			accumulator: AtomicU64::new(0),
			send_to_manager_tx,
			channel_to_entity_tx: None
		}
	}


	pub fn start(&mut self) -> mpsc::Sender<JsonValue> {
		log::debug!("开始进行实体的启动操作。启动消息接收。id:{}", self.id);

		let (manage_to_entity_tx, manage_to_entity_rx) = mpsc::channel(0x50);
		let (channel_to_entity_tx, channel_to_entity_rx) = mpsc::channel(0xffffffff);

		log::info!("通道{},,开始启动处理消息.", self.name);
		//这里开始自己的消息处理
		get_runtime().spawn(start_entity(
			manage_to_entity_rx, 
			channel_to_entity_rx, 
			self.id, 
			self.service_id.clone(), 
			self.sp_id.clone(),
			0, 
			self.now_channel_number.clone(), 
			EntityType::Custom,
			0,
		));

		self.channel_to_entity_tx = Some(channel_to_entity_tx);

		manage_to_entity_tx
	}
}

#[async_trait]
impl Entity for CustomEntity {
	async fn login_attach(&self, can_write: bool) -> (usize, SmsStatus, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>) {
		if self.max_channel_number <= self.now_channel_number.load(Ordering::Relaxed) as usize {
			log::warn!("当前已经满。不再继续增加。entity_id:{}", self.id);
			return (0, SmsStatus::OtherError, 0, 0, None, None, None);
		}

		//通过后进行附加上去的动作。
		let (entity_to_channel_priority_tx, entity_to_channel_priority_rx) = mpsc::channel(CHANNEL_BUFF_NUM as usize);
		let (entity_to_channel_common_tx, entity_to_channel_common_rx) = mpsc::channel(CHANNEL_BUFF_NUM as usize);
		let channel_to_entity_tx = self.channel_to_entity_tx.as_ref().unwrap().clone();

		// item.is_active = true;
		// item.can_write = true;
		// item.entity_to_channel_priority_tx = Some(entity_to_channel_priority_tx);
		// item.entity_to_channel_common_tx = Some(entity_to_channel_common_tx);
		let index = get_sequence_id(1);

		let mut save = TEMP_SAVE.write().await;
		save.insert(index, (entity_to_channel_priority_tx, entity_to_channel_common_tx));
		let msg = json::object! {
			msg_type : "Connect",
			entity_id : self.id,
			channel_id : index,
			can_write: can_write,
		};

		if let Err(e) = channel_to_entity_tx.send(msg).await {
			log::error!("发送消息出现异常。e:{}", e);
		}

		(index as usize, SmsStatus::Success, self.read_limit, self.write_limit, Some(entity_to_channel_priority_rx), Some(entity_to_channel_common_rx), Some(channel_to_entity_tx))
	}

	fn get_id(&self) -> u32 {
		self.id
	}

	fn get_allow_ips(&self) -> &str {
		self.allowed_addr.as_str()
	}

	fn get_entity_type(&self) -> EntityType {
		EntityType::Custom
	}

	fn can_login(&self) -> bool {
		true
	}

	fn get_login_name(&self) -> &str {
		self.login_name.as_str()
	}

	fn get_password(&self) -> &str {
		self.password.as_str()
	}
}