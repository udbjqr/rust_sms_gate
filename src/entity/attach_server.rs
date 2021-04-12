use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::atomic::Ordering::Relaxed;

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::mpsc::{self};

use crate::entity::{Entity, start_entity};
use crate::entity::channel::Channel;
use crate::get_runtime;
use crate::protocol::{SmsStatus, Protocol};
use crate::global::{TEMP_SAVE, get_sequence_id};

#[derive(Debug)]
pub struct ServerEntity {
	id: u32,
	name: String,
	login_name: String,
	password: String,
	addr: String,
	version: u32,
	read_limit: u32,
	write_limit: u32,
	max_channel_number: usize,
	now_channel_number: Arc<AtomicU8>,
	config: JsonValue,
	entity_to_manager_tx: mpsc::Sender<JsonValue>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
	is_active: Arc<AtomicBool>,
	protocol: Protocol,
	service_id: String,
	sp_id: String,
}

impl ServerEntity {
	pub fn new(id: u32,
	           name: String,
	           service_id: String,
	           sp_id: String,
	           login_name: String,
	           password: String,
	           addr: String,
	           version: u32,
	           protocol: String,
	           read_limit: u32,
	           write_limit: u32,
	           max_channel_number: usize,
	           config: JsonValue,
	           send_to_manager_tx: mpsc::Sender<JsonValue>,
	) -> Self {
		let protocol = Protocol::get_protocol(protocol.as_str(), version);

		ServerEntity {
			id,
			name,
			service_id,
			sp_id,
			login_name,
			password,
			addr,
			version,
			read_limit,
			write_limit,
			max_channel_number,
			now_channel_number: Arc::new(AtomicU8::new(0)),
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

		let (manage_to_entity_tx, manage_to_entity_rx) = mpsc::channel(0xff);
		let (channel_to_entity_tx, channel_to_entity_rx) = mpsc::channel(self.read_limit as usize);

		log::info!("通道{},,开始启动处理消息.", self.name);
		//这里开始自己的消息处理
		get_runtime().spawn(start_entity(manage_to_entity_rx, channel_to_entity_rx, self.id, self.service_id.clone(), self.sp_id.clone(), self.now_channel_number.clone()));

		self.channel_to_entity_tx = Some(channel_to_entity_tx);

		self.continued_connect(manage_to_entity_tx.clone());

		manage_to_entity_tx
	}

	///启动连接过程。如果连接不满。一直进行连接
	fn continued_connect(&self, manage_to_entity_tx: mpsc::Sender<JsonValue>) {
		let protocol = self.protocol.clone();
		let is_active = self.is_active.clone();
		let user_name = self.login_name.clone();
		let password = self.password.clone();
		let addr = self.addr.clone();
		let version = self.version.clone();
		let id = self.id.clone();
		let max_num = self.max_channel_number.clone();
		let now_num = self.now_channel_number.clone();

		get_runtime().spawn(async move {
			let login_msg = json::object! {
				login_name: user_name,
				password: password,
				address: addr,
				version: version,
				msg_type: "Connect"
			};

			//每10秒进行判断是否需要进行连接
			while is_active.load(Relaxed) {
				if manage_to_entity_tx.is_closed() {
					log::info!("当前实体已经退出.退出连接循环.id:{}", id);
					return;
				}

				let conn_num = max_num - now_num.load(Relaxed) as usize;
				if conn_num > 0 {
					for _item in 0..conn_num {
						let protocol = protocol.clone();
						let login_msg = login_msg.clone();
						get_runtime().spawn(async move {
							let mut channel = Channel::new(protocol, false);

							//启动连接准备
							if let Err(e) = channel.start_connect(id, login_msg).await {
								log::error!("连接服务端出现异常。。e:{}", e);
							}
						});
					};

					tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
				}
			}
		});
	}
}


#[async_trait]
impl Entity for ServerEntity {
	async fn login_attach(&self) -> (usize, SmsStatus, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>) {
		if self.max_channel_number <= self.now_channel_number.load(Ordering::Relaxed) as usize {
			log::warn!("当前已经满。不再继续增加。entity_id:{}", self.id);
			return (0, SmsStatus::OtherError, 0, 0, None, None, None);
		}

		//通过后进行附加上去的动作。
		let (entity_to_channel_priority_tx, entity_to_channel_priority_rx) = mpsc::channel(0xffffffff);
		let (entity_to_channel_common_tx, entity_to_channel_common_rx) = mpsc::channel(0xffffffff);
		let channel_to_entity_tx = self.channel_to_entity_tx.as_ref().unwrap().clone();

		let index = get_sequence_id(1);
		let mut save = TEMP_SAVE.write().await;
		save.insert(index, (entity_to_channel_priority_tx, entity_to_channel_common_tx));


		let msg = json::object! {
			msg_type:"Connect",
			entity_id : self.id,
			channel_id : index,
		};

		if let Err(e) = channel_to_entity_tx.send(msg).await {
			log::error!("发送消息出现异常。e:{}", e);
		}

		(index as usize, SmsStatus::Success, self.read_limit, self.write_limit, Some(entity_to_channel_priority_rx), Some(entity_to_channel_common_rx), Some(channel_to_entity_tx))
	}

	fn get_id(&self) -> u32 {
		self.id
	}

	fn get_login_name(&self) -> &str {
		self.login_name.as_str()
	}

	fn get_password(&self) -> &str{
		self.password.as_str()
	}

	///对请求端来说。所有都允许
	fn get_allow_ips(&self) -> &str {
		"0.0.0.0"
	}
}

