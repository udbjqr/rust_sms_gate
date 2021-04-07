use std::sync::Arc;
use std::sync::atomic::{AtomicU64};

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc, RwLock};

use crate::entity::{ChannelStates, Entity, start_entity};
use crate::get_runtime;
use crate::protocol::{SmsStatus, Protocol};
use crate::protocol::names::VERSION;

#[derive(Debug)]
pub struct CustomEntity {
	id: u32,
	name: String,
	desc: String,
	login_name: String,
	password: String,
	allowed_addr: Vec<String>,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: usize,
	channels: Arc<RwLock<Vec<ChannelStates>>>,
	config: JsonValue,
	accumulator: AtomicU64,
	send_to_manager_tx: mpsc::Sender<JsonValue>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
}

impl CustomEntity {
	pub fn new(id: u32,
	           name: String,
	           desc: String,
	           login_name: String,
	           password: String,
	           allowed_addr: Vec<String>,
	           read_limit: u32,
	           write_limit: u32,
	           max_channel_number: usize,
	           config: JsonValue,
	           send_to_manager_tx: mpsc::Sender<JsonValue>,
	) -> Self {
		let mut channels: Vec<ChannelStates> = Vec::with_capacity(max_channel_number);

		for _ in 0..max_channel_number {
			channels.push(ChannelStates::new());
		}

		CustomEntity {
			id,
			name,
			desc,
			login_name,
			password,
			allowed_addr,
			read_limit,
			write_limit,
			max_channel_number,
			channels: Arc::new(RwLock::new(channels)),
			config,
			accumulator: AtomicU64::new(0),
			send_to_manager_tx,
			channel_to_entity_tx: None,
		}
	}


	pub fn start(&mut self) -> mpsc::Sender<JsonValue> {
		log::debug!("开始进行实体的启动操作。启动消息接收。id:{}", self.id);

		let (manage_to_entity_tx, manage_to_entity_rx) = mpsc::channel(0xFFFFFFFF);
		let (channel_to_entity_tx, channel_to_entity_rx) = mpsc::channel(0xFFFFFFFF);

		log::info!("通道{},,开始启动处理消息.", self.name);
		//这里开始自己的消息处理
		get_runtime().spawn(start_entity(manage_to_entity_rx, channel_to_entity_rx, self.channels.clone(), self.id));

		self.channel_to_entity_tx = Some(channel_to_entity_tx);

		manage_to_entity_tx
	}
}

#[async_trait]
impl Entity for CustomEntity {
	async fn login_attach(&mut self, json: JsonValue) -> (usize, SmsStatus<JsonValue>, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>) {
		//这里已经开了锁。如果下面执行时间有点长。就需要注意。
		let mut channels = self.channels.write().await;
		let index = channels.iter().rposition(|i| i.is_active == false);

		if index.is_none() {
			log::warn!("当前已经满。不再继续增加。entity_id:{}", self.id);
			return (0, SmsStatus::LoginOtherError(json), 0, 0, None, None, None);
		}

		let index = index.unwrap();
		let item = channels.get_mut(index).unwrap();

		//通过后进行附加上去的动作。
		let (entity_to_channel_priority_tx, entity_to_channel_priority_rx) = mpsc::channel::<JsonValue>(5);
		let (entity_to_channel_common_tx, entity_to_channel_common_rx) = mpsc::channel::<JsonValue>(5);


		let channel_to_entity_tx = self.channel_to_entity_tx.as_ref().unwrap().clone();

		item.is_active = true;
		item.can_write = true;
		item.entity_to_channel_priority_tx = Some(entity_to_channel_priority_tx);
		item.entity_to_channel_common_tx = Some(entity_to_channel_common_tx);

		let msg = json::object! {
			manager_type:"create",
			entity_id : self.id,
			channel_id : index,
		};

		if let Err(e) = channel_to_entity_tx.send(msg).await {
			log::error!("发送消息出现异常。e:{}", e);
		}

		(index, SmsStatus::Success(json), self.read_limit, self.write_limit, Some(entity_to_channel_priority_rx), Some(entity_to_channel_common_rx), Some(channel_to_entity_tx))
	}

	fn get_id(&self) -> u32 {
		self.id
	}

	fn get_channels(&self) -> Arc<RwLock<Vec<ChannelStates>>> {
		self.channels.clone()
	}
}


pub fn check_custom_login<T>(json: &JsonValue, _entity: &mut Box<(dyn Entity + 'static)>, protocol: &T) -> Option<SmsStatus<JsonValue>>
	where T: Protocol {
	//TODO 进行地址允许判断
	//TODO 进行登录判断。
	//TODO 进行密码检验。

	//进行版本检查
	if let Some(version) = json[VERSION].as_u32() {
		if version != protocol.get_version() {
			return Some(SmsStatus::VersionError(json.clone()));
		}
	} else {
		log::error!("附加至CustomEntity通道异常。json里面没有version。。json:{}", json);
		return Some(SmsStatus::LoginOtherError(json.clone()));
	}

	None
}
