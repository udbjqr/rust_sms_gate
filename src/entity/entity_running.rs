use tokio::sync::{mpsc, RwLock};
use json::JsonValue;
use std::sync::Arc;
use crate::entity::{ChannelStates, EntityManager};
use std::collections::HashMap;
use crate::protocol::names::{MANAGER_TYPE, MSG_TYPE_STR, ID, WAIT_RECEIPT, SEQ_ID, LONG_SMS_TOTAL, SRC_ID, DEST_ID, DEST_IDS, LONG_SMS_NOW_NUMBER, MSG_ID, MSG_CONTENT, MSG_IDS, RECEIVE_TIME, ENTITY_ID, SEQ_IDS, CHANNEL_ID, IS_PRIORITY, NODE_ID, SERVICE_ID};
use crate::protocol::MsgType;
use crate::global::{message_sender, TOPIC_TO_B_SUBMIT, TOPIC_TO_B_DELIVER, TOPIC_TO_B_DELIVER_RESP, TOPIC_TO_B_SUBMIT_RESP, TOPIC_TO_B_REPORT_RESP, TOPIC_TO_B_REPORT, TOPIC_TO_B_FAILURE};
use crate::message_queue::KafkaMessageProducer;
use std::ops::{Add};

struct EntityRunContext {
	entity_id: u32,
	service_id: String,
	node_id: u32,
	index: usize,
	send_channels: Vec<ChannelStates>,
	wait_receipt_map: HashMap<u64, JsonValue>,
	long_sms_cache: HashMap<String, Vec<Option<JsonValue>>>,
	to_queue: Arc<KafkaMessageProducer>,
	manager_channels: Arc<RwLock<Vec<ChannelStates>>>,
}

pub async fn start_entity(mut manager_to_entity_rx: mpsc::Receiver<JsonValue>, mut from_channel: mpsc::Receiver<JsonValue>, channels: Arc<RwLock<Vec<ChannelStates>>>, entity_id: u32, service_id: String, node_id: u32) {
	let mut context = EntityRunContext {
		entity_id,
		service_id,
		node_id,
		index: 0,
		send_channels: Vec::new(),
		wait_receipt_map: HashMap::new(),
		long_sms_cache: HashMap::new(),
		to_queue: message_sender().clone(),
		manager_channels: channels,
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
				if !handle_from_manager_rx(from_manager_msg,&mut context).await{
					return;
				}
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
			msg[ENTITY_ID] = context.entity_id.into();

			match msg[MSG_TYPE_STR].as_str() {
				Some(v) => {
					match (v.into(), msg[WAIT_RECEIPT].as_bool()) {
						//需要等待回执
						(MsgType::Deliver, Some(true)) |
						(MsgType::Submit, Some(true)) |
						(MsgType::Report, Some(true)) => {
							insert_into_wait_receipt(&mut context.wait_receipt_map, msg);
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

							//保留除当前通道外其余通道
							context.send_channels.retain(|item| item.id != id);
							let mut channels = context.manager_channels.write().await;
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

fn get_key(msg: &JsonValue) -> u64 {
	if let Some(id) = msg[SEQ_ID].as_u64() {
		id
	} else {
		log::error!("get_key()收到的消息里面不包含seq_id.记录，并返回0.msg:{}", msg);
		0
	}
}

fn insert_into_wait_receipt(wait_receipt_map: &mut HashMap<u64, JsonValue>, json: JsonValue) {
	//如果是长短信。目前只放一条。如果一条收到，就算是全都收到了。
	//如果当前条未收到。准备全部进行重发。
	if json[SEQ_IDS].is_array() && !json[SEQ_IDS].is_empty() {
		if let Some(seq_id) = json[SEQ_IDS][0].as_u64() {
			wait_receipt_map.insert(seq_id, json);
		} else {
			log::error!("insert_into_wait_receipt出现错误。SEQ_IDS内为空...json:{}", json);
		}
	} else {
		log::error!("insert_into_wait_receipt出现错误。当前未取到SEQ_IDS...json:{}", json);
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
			Some(msg_id) => msg_id as u64
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


///entity处理来自于管理器端的消息
async fn handle_from_manager_rx(msg: Option<JsonValue>, context: &mut EntityRunContext) -> bool {
	match msg {
		None => {
			log::info!("发送端已经退出。直接退出。");
			return false;
		}
		Some(msg) => {
			match msg[MANAGER_TYPE].as_str() {
				Some("create") => {
					let entity_id = msg[ENTITY_ID].as_u32().unwrap();
					let channel_id = msg[CHANNEL_ID].as_usize().unwrap();
					let manager = EntityManager::get_entity_manager();
					let entitys = manager.entitys.read().await;

					if let Some(entity) = entitys.get(&entity_id) {
						let entity = entity.get_channels();
						let entity = entity.read().await;
						if let Some(channel) = entity.get(channel_id) {
							context.send_channels.push(ChannelStates {
								id: channel_id,
								is_active: true,
								can_write: true,
								entity_to_channel_priority_tx: Some(channel.entity_to_channel_priority_tx.as_ref().unwrap().clone()),
								entity_to_channel_common_tx: Some(channel.entity_to_channel_common_tx.as_ref().unwrap().clone()),
							});
						} else {
							log::error!("没有找到对应的channel..msg:{}", msg);
						}
					} else {
						log::error!("没有找到对应的entity..msg:{}", msg);
					}
				}
				Some("close") => {
					log::debug!("开始进行实体的关闭操作。");

					for channel in context.send_channels.iter() {
						if let Some(sender) = &channel.entity_to_channel_priority_tx {
							if let Err(e) = sender.send(json::parse(r#"{msg_type:"close"}"#).unwrap()).await {
								log::error!("发送关闭消息出现异常.e:{}", e);
							} else {
								log::error!("当前通道的发送队列为空...entity:{}", context.entity_id);
							}
						}
					}
					return false;
				}
				Some("send") => {
					send_to_channels(msg, context).await
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

	true
}

async fn send_to_channels(msg: JsonValue, context: &mut EntityRunContext) {
	//有可以选择发送的通道
	let mut send_msg = msg;

	if context.node_id > 0 {
		send_msg[NODE_ID] = context.node_id.into();
	}
	if !context.service_id.is_empty(){
		send_msg[SERVICE_ID] = context.service_id.as_str().into();
	}

	while !context.send_channels.is_empty() {
		let mut failure = None;
		if context.index == usize::max_value() {
			context.index = 1;
		}
		context.index += 1;

		let index = context.index % context.send_channels.len();
		{
			//选择到某一个通道.
			let select = match context.send_channels.get(index) {
				None => {
					if let Some(se) = context.send_channels.get(0) {
						se
					} else {
						log::error!("选择不到可以发送消息的通道..id:{}", context.entity_id);
						//一个通道都没有的时候.返回错误
						context.to_queue.send(TOPIC_TO_B_FAILURE, "", &send_msg.to_string()).await;
						return;
					}
				}

				Some(se) => se
			};

			if send_msg[IS_PRIORITY].as_bool().unwrap_or(false) {
				if let Some(sender) = &select.entity_to_channel_priority_tx {
					if let Err(e) = sender.send(send_msg).await {
						log::error!("发送正常消息出现异常.e:{}", e);

						failure = Some((e.0, select.id));
					}
				}
			} else {
				if let Some(sender) = &select.entity_to_channel_common_tx {
					if let Err(e) = sender.send(send_msg).await {
						log::error!("发送正常消息出现异常.e:{}", e);

						failure = Some((e.0, select.id));
					}
				}
			}
		}

		if let Some((json, channel_id)) = failure {
			// 对端可能已经关闭.当前的通道已经不可用,删除通道并且重选.
			context.send_channels.remove(index);
			{
				let mut channels = context.manager_channels.write().await;
				if let Some(mut item) = channels.get_mut(channel_id) {
					item.can_write = false;
					item.is_active = false;
					item.entity_to_channel_common_tx = None;
					item.entity_to_channel_priority_tx = None;
				}
			}

			send_msg = json;
		} else {
			return;
		}
	}
}

