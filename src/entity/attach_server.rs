use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;

use crate::entity::{ChannelStates, Entity, start_entity};
use crate::entity::channel::Channel;
use crate::get_runtime;
use crate::protocol::{Cmpp48, Sgip, SmsStatus};

#[derive(Debug)]
pub struct ServerEntity {
	id: u32,
	name: String,
	login_name: String,
	password: String,
	addr: String,
	version: String,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: usize,
	channels: Arc<RwLock<Vec<ChannelStates>>>,
	config: JsonValue,
	entity_to_manager_tx: mpsc::Sender<JsonValue>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
	is_active: Arc<AtomicBool>,
	protocol: String,
}

impl ServerEntity {
	pub fn new(id: u32,
	           name: String,
	           login_name: String,
	           password: String,
	           addr: String,
	           version: String,
	           protocol: String,
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


		ServerEntity {
			id,
			name,
			login_name,
			password,
			addr,
			version,
			read_limit,
			write_limit,
			max_channel_number,
			channels: Arc::new(RwLock::new(channels)),
			config,
			entity_to_manager_tx: send_to_manager_tx,
			channel_to_entity_tx: None,
			is_active: Arc::new(AtomicBool::new(true)),
			protocol,
		}
	}
	///开启一个entity的启动。连接对端,准备接收数据等
	pub async fn start(&mut self) -> mpsc::Sender<JsonValue> {
		log::debug!("开始进行实体的启动操作。启动消息接收。id:{}", self.id);

		let (manage_to_entity_tx, manage_to_entity_rx) = mpsc::channel(0xffffffff);
		let (channel_to_entity_tx, channel_to_entity_rx) = mpsc::channel(0xff);

		//这里开始自己的消息处理
		get_runtime().spawn(start_entity(manage_to_entity_rx, channel_to_entity_rx, self.channels.clone()));

		self.channel_to_entity_tx = Some(channel_to_entity_tx);

		self.continued_connect();

		manage_to_entity_tx
	}

	///启动连接过程。如果连接不满。一直进行连接
	fn continued_connect(&self) {
		let channels = self.channels.clone();
		let protocol_name = self.protocol.clone();
		let is_active = self.is_active.clone();
		let addr = self.addr.clone();
		let user_name = self.login_name.clone();
		let password = self.password.clone();
		let version = self.version.clone();
		let id = self.id.clone();

		get_runtime().spawn(async move {
			//每10秒进行判断是否需要进行连接
			while is_active.load(Relaxed) {
				let channels = channels.read().await;

				for item in channels.iter() {
					if item.is_active == false {
						match protocol_name.as_str() {
							//TODO 根据协议生成对象
							"cmpp48" => {
								let mut channel = Channel::new(Cmpp48::new(), false);
								if let Err(e) = channel.start_connect(id, addr.as_str(), user_name.as_str(), password.as_str(), version.as_str()).await {
									log::error!("连接服务端出现异常。。e:{}", e);
								}
							}
							"sgip" => {
								let mut channel = Channel::new(Sgip::new(), false);
								if let Err(e) = channel.start_connect(id, addr.as_str(), user_name.as_str(), password.as_str(), version.as_str()).await {
									log::error!("连接服务端出现异常。。e:{}", e);
								}
							}
							_ => {}
						}
					}
				};

				tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
			}
		});
	}
}


#[async_trait]
impl Entity for ServerEntity {
	async fn login_attach(&mut self, json: JsonValue) -> (usize, SmsStatus<JsonValue>, u32, u32, Option<Receiver<JsonValue>>, Option<Receiver<JsonValue>>, Option<Sender<JsonValue>>) {
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
			msg_type:"create",
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
