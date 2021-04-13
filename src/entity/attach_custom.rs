use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc};

use crate::entity::{Entity, start_entity, EntityType};
use crate::get_runtime;
use crate::protocol::{SmsStatus, Protocol};
use crate::protocol::names::{VERSION, AUTHENTICATOR, TIMESTAMP};
use std::net::{Ipv4Addr, IpAddr};
use crate::global::{TEMP_SAVE, get_sequence_id};

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
	           send_to_manager_tx: mpsc::Sender<JsonValue>,
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
			channel_to_entity_tx: None,
		}
	}


	pub fn start(&mut self) -> mpsc::Sender<JsonValue> {
		log::debug!("开始进行实体的启动操作。启动消息接收。id:{}", self.id);

		let (manage_to_entity_tx, manage_to_entity_rx) = mpsc::channel(0x50);
		let (channel_to_entity_tx, channel_to_entity_rx) = mpsc::channel(0x50);

		log::info!("通道{},,开始启动处理消息.", self.name);
		//这里开始自己的消息处理
		get_runtime().spawn(start_entity(manage_to_entity_rx, channel_to_entity_rx, self.id, self.service_id.clone(), self.sp_id.clone(), self.now_channel_number.clone(), EntityType::Custom));

		self.channel_to_entity_tx = Some(channel_to_entity_tx);

		manage_to_entity_tx
	}
}

#[async_trait]
impl Entity for CustomEntity {
	async fn login_attach(&self) -> (usize, SmsStatus, u32, u32, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Receiver<JsonValue>>, Option<mpsc::Sender<JsonValue>>) {
		if self.max_channel_number <= self.now_channel_number.load(Ordering::Relaxed) as usize {
			log::warn!("当前已经满。不再继续增加。entity_id:{}", self.id);
			return (0, SmsStatus::OtherError, 0, 0, None, None, None);
		}

		// //这里已经开了锁。如果下面执行时间有点长。就需要注意。
		// let mut channels = self.channels.write().await;
		// let index = channels.iter().rposition(|i| i.is_active == false);
		//
		// if index.is_none() {
		//
		// }
		//
		// let index = index.unwrap();
		// let item = channels.get_mut(index).unwrap();

		//通过后进行附加上去的动作。
		let (entity_to_channel_priority_tx, entity_to_channel_priority_rx) = mpsc::channel(0xffffffff);
		let (entity_to_channel_common_tx, entity_to_channel_common_rx) = mpsc::channel(0xffffffff);
		let channel_to_entity_tx = self.channel_to_entity_tx.as_ref().unwrap().clone();

		// item.is_active = true;
		// item.can_write = true;
		// item.entity_to_channel_priority_tx = Some(entity_to_channel_priority_tx);
		// item.entity_to_channel_common_tx = Some(entity_to_channel_common_tx);
		let index = get_sequence_id(1);

		let mut save = TEMP_SAVE.write().await;
		save.insert(index, (entity_to_channel_priority_tx, entity_to_channel_common_tx));
		let msg = json::object! {
			msg_type : "create",
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

	fn get_password(&self) -> &str {
		self.password.as_str()
	}

	fn get_allow_ips(&self) -> &str {
		self.allowed_addr.as_str()
	}
}


pub fn check_custom_login(json: &JsonValue, entity: &Box<(dyn Entity + 'static)>, protocol: &mut Protocol, ip_addr: IpAddr) -> Option<(SmsStatus, JsonValue)> {
	//进行地址允许判断
	if !check_addr_range(entity.get_allow_ips(), ip_addr) {
		return Some((SmsStatus::AddError, json.clone()));
	}

	// 进行密码检验。
	let my_auth = protocol.get_auth(entity.get_login_name(), entity.get_password(), json[TIMESTAMP].as_u32().unwrap_or(0));

	let (_, my_auth) = my_auth.split_at(8);
	let auth = u64::from_be_bytes(unsafe { *(my_auth as *const _ as *const [u8; 8]) });

	if auth != json[AUTHENTICATOR].as_u64().unwrap_or(0) {
		return Some((SmsStatus::AuthError, json.clone()));
	}

	//进行版本检查.并且根据对方版本号进行修改
	if let Some(version) = json[VERSION].as_u32() {
		if !protocol.has(version) {
			return Some((SmsStatus::VersionError, json.clone()));
		} else {
			*protocol = protocol.match_version(version);
		}
	} else {
		log::error!("附加至CustomEntity通道异常。json里面没有version。。json:{}", json);
		return Some((SmsStatus::OtherError, json.clone()));
	}

	None
}


///检查ip地址是否在允许的范围内。true 在。false 不在
fn check_addr_range(allow_ips: &str, now_ip: IpAddr) -> bool {
	log::trace!("进行地址检查.来源地址:{}..允许的地址.{}", now_ip, allow_ips);
	//ipv6目前不处理。
	let ip_v = match now_ip {
		IpAddr::V4(ip) => {
			ip.octets()
		}
		IpAddr::V6(_) => {
			return false;
		}
	};

	allow_ips.split(",").find(|addr| {
		if let Ok(addr) = addr.parse::<Ipv4Addr>() {
			let v = addr.octets();
			let mut found = true;
			for ind in 0..v.len() {
				found = v[ind] == ip_v[ind] || v[ind] == 0;
				if !found { return false; }
			}

			found
		} else {
			false
		}
	}).is_some()
}
