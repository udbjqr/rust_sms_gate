use std::collections::HashMap;
use std::sync::Arc;

use json::JsonValue;
use log::{error, info};
use rdkafka::{ClientConfig, Message};
use rdkafka::config::RDKafkaLogLevel;
use rdkafka::consumer::{Consumer, StreamConsumer};
use tokio::io;
use tokio::sync::{mpsc, RwLock};

use crate::entity::{CustomEntity, Entity};
use crate::entity::as_server::ServerEntity;
use crate::get_runtime;
use crate::global::{load_config_file, TOPIC_FROM_B_SUBMIT, TOPIC_FROM_B_DELIVER, TOPIC_FROM_B_REPORT, TOPIC_PASSAGE_REQUEST_STATE};
use crate::protocol::names::{ADDRESS, ALLOW_ADDRS, CROP_ID, GATEWAY_LOGIN_NAME, GATEWAY_PASSWORD, LOGIN_NAME, MANAGER_TYPE, MAX_CHANNEL_NUMBER, NAME, NODE_ID, OP_NAME, PASSWORD, PROTOCOL, READ_LIMIT, SERVICE_ID, SP_ID, VERSION, WRITE_LIMIT};

///实体的管理对象。
/// 负责处理消息队列送过来的实体的开启、关闭等操作
#[derive(Debug)]
pub struct EntityManager {
	is_active: bool,
	pub entitys: Arc<RwLock<HashMap<u32, Box<dyn Entity>>>>,
}

static mut ENTITY_MANAGER: Option<Arc<EntityManager>> = None;

impl EntityManager {
	pub fn get_entity_manager() -> Arc<EntityManager> {
		unsafe {
			ENTITY_MANAGER.get_or_insert_with(|| {
				let mut result = EntityManager {
					is_active: false,
					entitys: Arc::new(RwLock::new(HashMap::new())),
				};

				if let Err(e) = result.start() {
					log::error!("启动出现异常.e:{}", e);
				}

				Arc::new(result)
			}).clone()
		}
	}

	///启动接收消息服务。
	pub fn start(&mut self) -> Result<(), io::Error> {
		if self.is_active {
			log::error!("已经启动。不能再次启动。");
			return Err(io::Error::new(io::ErrorKind::AlreadyExists, "已经启动。不能再次启动。"));
		}

		log::info!("开始接收消息服务");

		let config = load_config_file("config/message_receiver.json");

		let d = vec![
			"account.modify",
			"account.add",
			"account.remove",
			"passage.modify",
			"passage.add",
			"passage.remove",
			"passage.init",
			"account.init",
			TOPIC_PASSAGE_REQUEST_STATE,
			TOPIC_FROM_B_SUBMIT,
			TOPIC_FROM_B_DELIVER,
			TOPIC_FROM_B_REPORT,
		];

		//定义来自于服务器的消息队列
		let from_servers: StreamConsumer = match ClientConfig::new()
			.set_log_level(RDKafkaLogLevel::Info)
			.set("group.instance.id", config["group"]["instance"]["id"].as_str().unwrap().to_owned())
			.set("group.id", config["group"]["id"].as_str().unwrap().to_owned())
			.set("bootstrap.servers", config["bootstrap"]["servers"].as_str().unwrap().to_owned())
			.set("session.timeout.ms", "6000")
			.set("auto.offset.reset", "earliest")
			.set("enable.auto.commit", "true")
			.set("auto.commit.interval.ms", "2000")
			// .set("enable.auto.offset.store", "false")
			.create() {
			Ok(v) => v,
			Err(e) => {
				log::error!("初始化消息队列出现异常,退出。e:{}", e);
				return Err(io::Error::new(io::ErrorKind::NotConnected, e));
			}
		};

		if let Err(e) = from_servers.subscribe(&d) {
			log::error!("启动接收程序出现异常,退出。e:{}", e);
			return Err(io::Error::new(io::ErrorKind::Other, e));
		}

		get_runtime().spawn(start_server(from_servers));

		self.is_active = true;

		Ok(())
	}
}

struct RunContext {
	entity_to_manager_tx: mpsc::Sender<JsonValue>,
	senders: HashMap<u32, mpsc::Sender<JsonValue>>,
}

pub async fn start_server(from_servers: StreamConsumer) {
	let (give_entity, mut from_entity) = mpsc::channel::<JsonValue>(0xffffffff);
	let senders = HashMap::new();

	let mut context = RunContext { entity_to_manager_tx: give_entity, senders };
	loop {
		tokio::select! {
			biased;

			//接收来自于实体对象的消息。一般来说都是实体对象状态改变
			json = from_entity.recv() => {
				match json {
					None => {
						log::warn!("对端已经关闭发送消息。结束退出!");
						return;
					}
					Some(msg) => {
						if let Some(manager_type) = msg[MANAGER_TYPE].as_str() {
							handle_from_entity_msg(manager_type, &msg, &mut context).await;
						} else {
							log::error!("接收的消息里面不包含manager_type.. msg:{}", msg);
						}
					}
				}
			}
			msg = from_servers.recv() => {
				match msg {
					Ok(msg) => {
						let body = match msg.payload_view::<str>() {
							Some(Ok(body)) => {
								info!("收到business消息:topic:{},msg:{}",msg.topic(), body);
								body
							},
							Some(Err(e)) => {
								error!("获得数据出现错误:{}", e);
								continue;
							}
							None => {
								log::warn!("解析结果为null");
								continue;
							}
						};

						let json = match json::parse(body) {
							Ok(json) => json,
							Err(e) => {
								error!("解码json格式出现错误:{}.文本:{}", e, body);
								continue;
							}
						};

						handle_queue_msg(msg.topic(),json,&mut context).await;
					}
					Err(e) => {
						log::error!("接收服务器消息出现异常。e:{}",e);
					}
				}
			}
		}
	}
}

///处理从实体过来的消息。
async fn handle_from_entity_msg(manager_type: &str, msg: &JsonValue, context: &mut RunContext) {
	let id = match msg["id"].as_u32() {
		None => {
			log::error!("未在消息里面找到id。msg:{}", msg);
			return;
		}
		Some(id) => id,
	};

	let entity_sender = match context.senders.get(&id) {
		None => {
			log::error!("未在发送列表里面找到id对应的对象。id:{}", id);
			return;
		}
		Some(v) => v,
	};

	match manager_type {
		"close" => {
			let close_json = json::object! {manager_type:"close"};
			if let Err(e) = entity_sender.send(close_json.clone()).await {
				log::error!("发送消息出现异常。对端可能已经关闭。e:{}", e);
			}
			context.senders.remove(&id);
		}
		//继续收到entity来的消息
		_ => {
			log::error!("收到一个未知的manager_type.不处理。跳过。msg:{}", msg);
		}
	}
}

///处理从消息队列过来的实体相关的消息。
async fn handle_queue_msg(topic: &str, mut json: JsonValue, context: &mut RunContext) {
	let id = match json["id"].as_u32() {
		None => {
			0
		}
		Some(v) => v
	};

	let entity_manager = EntityManager::get_entity_manager();
	match topic {
		"send.submit" | "send.deliver" | "send.report" => {
			if let Some(sender) = context.senders.get(&id) {
				json[MANAGER_TYPE] = "send".into();
				if let Err(e) = sender.send(json).await {
					log::error!("发送出现异常.e:{}", e);
				}
			} else {
				log::error!("未找到指定id的实体发送者,跳过。msg:{}", json);
			}
		}
		"passage.add" | "account.add" | "passage.modify" | "passage.init" | "account.init" | "account.modify" => {
			let mut entitys = entity_manager.entitys.write().await;
			if let Some(_) = entitys.get(&id) {
				if let Some(sender) = context.senders.remove(&id) {
					let close_json = json::object! {manager_type:"close"};
					if let Err(e) = sender.send(close_json.clone()).await {
						log::warn!("向entity发送关闭操作失败。e:{}", e);
					}
				} else {
					log::warn!("未在发送队列里面找到发送者。不发送。id:{}", id);
				}
				entitys.remove(&id);
			};

			if topic.starts_with("account") {
				let mut entity = Box::new(CustomEntity::new(
					id,
					json[NAME].as_str().unwrap_or("未知").to_string(),
					json[SERVICE_ID].as_str().unwrap_or("未知").to_string(),
					json[SP_ID].as_str().unwrap_or("").to_string(),
					json["desc"].as_str().unwrap_or("").to_string(),
					json[LOGIN_NAME].as_str().unwrap_or("").to_string(),
					json[PASSWORD].as_str().unwrap_or("").to_string(),
					json[ALLOW_ADDRS].as_str().unwrap_or("").to_string(),
					json[READ_LIMIT].as_u32().unwrap_or(200),
					json[WRITE_LIMIT].as_u32().unwrap_or(200),
					json[MAX_CHANNEL_NUMBER].as_usize().unwrap_or(0xff),
					json,
					context.entity_to_manager_tx.clone(),
				));

				let send_to_entity = entity.start();

				//放入管理队列的数据
				context.senders.insert(entity.get_id(), send_to_entity);
				entitys.insert(entity.get_id(), entity);
			} else {
				if json[ADDRESS].as_str().unwrap_or("").to_string() == "" {
					log::error!("出现一个错误。没有等连接的IP地址。json:{}",&json);
					return 
				}

				let protocol_name = json[PROTOCOL].as_str().unwrap_or("").to_string();
				let sp_id = if protocol_name == "SGIP" {
					log::trace!("SGIP协议，使用sp_id字段存放crop_id");

					json[CROP_ID].as_str().unwrap_or("").to_string()
				} else {
					json[SP_ID].as_str().unwrap_or("").to_string()
				};

				let mut entity = Box::new(ServerEntity::new(
					id,
					json[OP_NAME].as_str().unwrap_or("未知").to_string(),
					json[SERVICE_ID].as_str().unwrap_or("未知").to_string(),
					sp_id,
					json[LOGIN_NAME].as_str().unwrap_or("").to_string(),
					json[PASSWORD].as_str().unwrap_or("").to_string(),
					json[ADDRESS].as_str().unwrap_or("").to_string(),
					json[VERSION].as_str().unwrap_or("0").parse().unwrap_or(0),
					protocol_name,
					json[READ_LIMIT].as_u32().unwrap_or(200),
					json[WRITE_LIMIT].as_u32().unwrap_or(200),
					json[MAX_CHANNEL_NUMBER].as_usize().unwrap_or(0x1),
					json[GATEWAY_LOGIN_NAME].as_str().unwrap_or("").to_string(),
					json[GATEWAY_PASSWORD].as_str().unwrap_or("").to_string(),
					json[NODE_ID].as_str().unwrap_or("0").parse().unwrap_or(0),
					json,
					context.entity_to_manager_tx.clone(),
				));

				let send_to_entity = entity.start().await;

				//放入管理队列的数据
				context.senders.insert(entity.get_id(), send_to_entity);
				entitys.insert(entity.get_id(), entity);
			}
		}
		"account.remove" | "passage.remove" => {
			let mut entitys = entity_manager.entitys.write().await;
			if let Some(_) = entitys.get(&id) {
				if let Some(sender) = context.senders.remove(&id) {
					let close_json = json::object! {manager_type:"close"};
					if let Err(e) = sender.send(close_json.clone()).await {
						log::warn!("向entity发送关闭操作失败。e:{}", e);
					}
				} else {
					log::warn!("未在发送队列里面找到发送者。不发送。id:{}", id);
				}
				entitys.remove(&id);
			}

			info!("收到{}消息。但现在不删除,只进行关闭操作。", topic);
		}
		//请求状态改变消息
		"passage.request.state" => {
			for (_, item) in context.senders.iter() {
				json[MANAGER_TYPE] = topic.into();
				if let Err(e) = item.send(json.clone()).await {
					log::error!("发送至entity出现异常.e:{}", e);
				}
			}
		}
		_ => {
			error!("接收到一个我也不知道的主题。退出。topic:{}", topic);
		}
	}
}
