use tokio::sync::{mpsc, RwLock};
use json::JsonValue;
use std::sync::Arc;
use crate::entity::{ChannelStates, EntityManager};
use std::collections::HashMap;
use crate::protocol::names::{MANAGER_TYPE, MSG_TYPE_STR, ID, WAIT_RECEIPT, SEQ_ID, LONG_SMS_TOTAL, SRC_ID, DEST_ID, DEST_IDS, LONG_SMS_NOW_NUMBER, MSG_ID, MSG_CONTENT, MSG_IDS, RECEIVE_TIME};
use crate::protocol::MsgType;
use crate::global::{message_sender, TOPIC_TO_B_SUBMIT, TOPIC_TO_B_DELIVER, TOPIC_TO_B_DELIVER_RESP, TOPIC_TO_B_SUBMIT_RESP, TOPIC_TO_B_REPORT_RESP, TOPIC_TO_B_REPORT};
use crate::message_queue::KafkaMessageProducer;
use std::ops::{Add};

struct EntityRunContext {
	channels_map: HashMap<usize, (mpsc::Sender<JsonValue>, mpsc::Sender<JsonValue>)>,
	wait_receipt_map: HashMap<u32, JsonValue>,
	long_sms_cache: HashMap<String, Vec<Option<JsonValue>>>,
	to_queue: Arc<KafkaMessageProducer>,
	channels: Arc<RwLock<Vec<ChannelStates>>>,
}

pub async fn start_entity(mut manager_to_entity_rx: mpsc::Receiver<JsonValue>, mut from_channel: mpsc::Receiver<JsonValue>, channels: Arc<RwLock<Vec<ChannelStates>>>) {
	let mut context = EntityRunContext {
		channels_map: HashMap::new(),
		wait_receipt_map: HashMap::new(),
		long_sms_cache: HashMap::new(),
		to_queue: message_sender().clone(),
		channels,
	};

	let mut timestamp = chrono::Local::now().timestamp();
	let duration = 86400;

	loop {
		//一个时间窗口过去,清除数据
		if (timestamp + duration) < chrono::Local::now().timestamp() {
			clear_long_sms_cache(&mut context.long_sms_cache);
			timestamp = chrono::Local::now().timestamp()
		}

		tokio::select! {
			from_manager_msg = manager_to_entity_rx.recv() => {
				handle_from_manager_rx(from_manager_msg,&mut context).await;
			}
			from_channel_msg = from_channel.recv() => {
				handle_from_channel_rx(from_channel_msg,&mut context).await;
			}
		}
	}
}

fn clear_long_sms_cache(cache: &mut HashMap<String, Vec<Option<JsonValue>>>) {
	let duration = 86400;
	let now = chrono::Local::now().timestamp();

	cache.retain(|_key, value| {
		//里面没有一个超时了.返回false
		value.iter().find(|item| {
			let mut found = false;
			if let Some(json) = item {
				dbg!(now,json[RECEIVE_TIME].as_i64(),);
				if json[RECEIVE_TIME].as_i64().unwrap_or(0) + duration <= now {
					found = true;
				}
			}

			found
		}).is_none()
	});
}

///entity处理来自于通道端的消息
async fn handle_from_channel_rx(msg: Option<JsonValue>, context: &mut EntityRunContext) {
	match msg {
		None => {
			log::info!("发送端已经全部退出。退出。");
			return;
		}
		Some(mut msg) => {
			msg[RECEIVE_TIME] = chrono::Local::now().timestamp().into();

			match msg[MSG_TYPE_STR].as_str() {
				Some(v) => {
					match (v.into(), msg[WAIT_RECEIPT].as_bool()) {
						//需要等待回执
						(MsgType::Deliver, Some(true)) |
						(MsgType::Submit, Some(true)) |
						(MsgType::Report, Some(true)) => {
							context.wait_receipt_map.insert(get_key(&msg), msg);
						}
						(MsgType::ReportResp, _) => {
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue(&context.to_queue, TOPIC_TO_B_REPORT_RESP, "", msg).await;
						}
						(MsgType::DeliverResp, _) => {
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue(&context.to_queue, TOPIC_TO_B_DELIVER_RESP, "", msg).await;
						}
						(MsgType::SubmitResp, _) => {
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue(&context.to_queue, TOPIC_TO_B_SUBMIT_RESP, "", msg).await;
						}
						//收到上传消息
						(MsgType::Report, Some(false)) |
						(MsgType::Report, None) => {
							send_to_queue(&context.to_queue, TOPIC_TO_B_REPORT, "", msg).await;
						}
						//收到上传消息
						(MsgType::Deliver, Some(false)) |
						(MsgType::Deliver, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(json) = handle_long_sms(context, msg, total) {
									send_to_queue(&context.to_queue, TOPIC_TO_B_DELIVER, "", json).await;
								}
							} else {
								send_to_queue(&context.to_queue, TOPIC_TO_B_DELIVER, "", msg).await;
							}
						}
						(MsgType::Submit, Some(false)) |
						(MsgType::Submit, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(json) = handle_long_sms(context, msg, total) {
									send_to_queue(&context.to_queue, TOPIC_TO_B_SUBMIT, "", json).await;
								}
							} else {
								send_to_queue(&context.to_queue, TOPIC_TO_B_SUBMIT, "", msg).await;
							}
						}
						(MsgType::Terminate, _) => {
							log::debug!("通道关闭操作。");

							let id = match msg[ID].as_usize() {
								None => {
									log::error!("未收到关闭请求过来的id。msg:{}", msg);
									return;
								}
								Some(v) => v
							};

							context.channels_map.remove(&id);
							let mut channels = context.channels.write().await;
							if let Some(mut item) = channels.get_mut(id) {
								item.can_write = false;
								item.is_active = false;
								item.entity_to_channel_common_tx = None;
								item.entity_to_channel_priority_tx = None;
							}
						}
						_ => {}
					}
				}
				None => {
					log::error!("未找到需要的msg_type_str内容。msg:{}", msg);
				}
			}
		}
	}
}

fn handle_long_sms(context: &mut EntityRunContext, msg: JsonValue, total: u8) -> Option<JsonValue> {
	let key = String::from(msg[SRC_ID].as_str().unwrap_or("")).
		add(msg[DEST_IDS][0].as_str().unwrap_or("")).
		add(msg[DEST_ID].as_str().unwrap_or(""));

	let index = match msg[LONG_SMS_NOW_NUMBER].as_u8() {
		None => {
			log::error!("没有当前是长短信的第几个的信息。不能放入。msg:{}", msg);
			return None;
		}
		Some(index) => {
			if index > total || total < 2 {
				log::error!("当前索引超过最大长度,或者总长度小于2。不能放入。msg:{}", msg);
				return None;
			}

			(index - 1) as usize
		}
	};

	let vec = match context.long_sms_cache.get_mut(&key) {
		None => {
			let mut vec = vec![None; total as usize];

			vec[index] = Some(msg);
			context.long_sms_cache.insert(key, vec);

			return None;
		}
		Some(vec) => {
			vec[index] = Some(msg);

			vec
		}
	};

	//没满,退出
	if let Some(_) = vec.iter().find(|json| json.is_none()) {
		return None;
	}

	//满了的处理方式
	let mut msg_ids = Vec::with_capacity(total as usize);
	let mut msg_content = String::new();

	for item in vec.iter() {
		let item = item.as_ref().unwrap();
		let msg_id = match item[MSG_ID].as_u32() {
			None => get_key(&item),
			Some(msg_id) => msg_id as u32
		};

		msg_ids.push(msg_id);
		msg_content.push_str(item[MSG_CONTENT].as_str().unwrap_or(""));
	}

	let mut json = vec.remove(0).unwrap();

	json[MSG_CONTENT] = msg_content.as_str().into();
	json[MSG_IDS] = msg_ids.into();
	json.remove(MSG_ID);

	context.long_sms_cache.remove(&key);

	Some(json)
}

async fn send_to_queue(to_queue: &Arc<KafkaMessageProducer>, topic: &str, key: &str, json: JsonValue) {
	if let Some(v) = json.as_str() {
		to_queue.send(topic, key, v).await;
	} else {
		log::error!("发送至消息队列出现异常。topic:{},key:{}.消息：{}", topic, key, json);
	}
}

fn get_key(json: &JsonValue) -> u32 {
	*json[SEQ_ID].as_u32().get_or_insert(0)
}

///entity处理来自于管理器端的消息
async fn handle_from_manager_rx(msg: Option<JsonValue>, context: &mut EntityRunContext) {
	match msg {
		None => {
			log::info!("发送端已经退出。直接退出。");
			return;
		}
		Some(msg) => {
			match msg[MANAGER_TYPE].as_str() {
				Some("create") => {
					let entity_id = msg["entity_id"].as_u32().unwrap();
					let channel_id = msg["channel_id"].as_usize().unwrap();
					let manager = EntityManager::get_entity_manager();
					let entitys = manager.entitys.read().await;

					if let Some(entity) = entitys.get(&entity_id) {
						let entity = entity.get_channels();
						let entity = entity.read().await;
						if let Some(channel) = entity.get(channel_id) {
							context.channels_map.insert(
								channel_id,
								(channel.entity_to_channel_priority_tx.as_ref().unwrap().clone(),
								 channel.entity_to_channel_common_tx.as_ref().unwrap().clone()),
							);
						} else {
							log::error!("没有找到对应的channel..msg:{}", msg);
						}
					} else {
						log::error!("没有找到对应的entity..msg:{}", msg);
					}
				}
				Some("close") => {
					log::debug!("开始进行实体的关闭操作。");

					let mut items = context.channels.write().await;
					for i in 0..items.len() {
						if let Some(sender) = context.channels_map.get(&i) {
							if let Err(e) = sender.0.send(json::parse(r#"{msg_type:"close"}"#).unwrap()).await {
								log::error!("发送关闭消息出现异常.e:{}", e);
							}
						} else {
							log::error!("未找到发送消息对应的id:id :{}", i);
						}

						let item = items.get_mut(i).unwrap();

						item.can_write = false;
						item.is_active = false;
						item.entity_to_channel_common_tx = None;
						item.entity_to_channel_priority_tx = None;
					}
				}

				None => {
					log::error!("接收消息出现异常。消息没有相关类型。msg:{}", msg);
				}
				_ => {
					log::error!("收到一个无法处理的类型。msg:{}", msg);
				}
			}
		}
	}
}

