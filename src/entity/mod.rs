use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr};

use async_trait::async_trait;
use json::JsonValue;
use tokio::sync::{mpsc};

use crate::protocol::names::{AUTHENTICATOR, TIMESTAMP, VERSION};
use crate::protocol::{Protocol, SmsStatus};

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
	///当前entity是否允许登录
	fn can_login(&self) -> bool;
	fn login(&self,json: &JsonValue, protocol: &mut Protocol, ip_addr: IpAddr) -> Option<(SmsStatus, JsonValue)>{
		//进行地址允许判断
		if !check_addr_range(self.get_allow_ips(), ip_addr) {
			return Some((SmsStatus::AddError, json.clone()));
		}
	
		// 进行密码检验。
		let my_auth = protocol.get_auth(self.get_login_name(), self.get_password(), json[TIMESTAMP].as_u32().unwrap_or(0));
	
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
}


///检查ip地址是否在允许的范围内。true 在。false 不在
fn check_addr_range(allow_ips: &str, now_ip: IpAddr) -> bool {
	log::debug!("进行地址检查.来源地址:{}..允许的地址.{}", now_ip, allow_ips);
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


