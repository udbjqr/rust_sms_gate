use tokio::sync::{mpsc};
use json::JsonValue;
use std::sync::Arc;
use crate::entity::{ChannelStates, EntityType};
use std::collections::HashMap;
use crate::protocol::names::{ACCOUNT_MSG_ID, MSG_TYPE_U32, CAN_WRITE, DEST_ID, DEST_IDS, DURATION, ENTITY_ID, ID, IS_PRIORITY, LONG_SMS_NOW_NUMBER, LONG_SMS_TOTAL, MANAGER_TYPE, MSG_CONTENT, MSG_ID, MSG_IDS, MSG_TYPE_STR, NEED_RE_SEND, NODE_ID, PASSAGE_MSG_ID, RECEIVE_TIME, SEQ_ID, SEQ_IDS, SERVICE_ID, SP_ID, SRC_ID, STATE, WAIT_RECEIPT};
use crate::protocol::MsgType;
use crate::global::{TEMP_SAVE, message_sender, TOPIC_TO_B_SUBMIT, TOPIC_TO_B_DELIVER, TOPIC_TO_B_DELIVER_RESP, TOPIC_TO_B_SUBMIT_RESP, TOPIC_TO_B_REPORT_RESP, TOPIC_TO_B_REPORT, TOPIC_TO_B_FAILURE, TOPIC_TO_B_PASSAGE_STATE_CHANGE, TOPIC_TO_B_ACCOUNT_STATE_CHANGE};
use crate::message_queue::KafkaMessageProducer;
use std::ops::{Add};
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering::SeqCst;
use std::fmt::{Display, Formatter, Error};
use std::result;

#[macro_use]
///把超时的消息进行重发
macro_rules! re_send {
  ($target: expr) => (
		// log::trace!("开始进行重发处理.{}",$target.entity_id);
		let mut re_sends = Vec::with_capacity($target.wait_receipt_map.len());
		let now = chrono::Local::now().timestamp();

		$target.wait_receipt_map.retain(|_, v| {
			let receive_time = v[RECEIVE_TIME].as_i64().unwrap_or(0);
			let duration = v[DURATION].as_i64().unwrap_or(30i64);

			//如果超时或者需要重发.
			if (receive_time + duration) < now && v[NEED_RE_SEND].as_bool().unwrap_or(true) {
				re_sends.push(v.to_owned());
				v[DURATION] = (duration + duration).into();

				false
			} else {
				true
			}
		});

		while let Some(msg) = re_sends.pop() {
			send_to_channels(msg.to_owned(), $target).await;
		}
  );
}

macro_rules! send_to_queue {
	($to_queue: expr, $topic: expr, $key: expr, $json: expr) => (
		$json.remove(MANAGER_TYPE);
		$json.remove(SEQ_ID);
		$json.remove(SEQ_IDS);
		$json.remove(MSG_TYPE_U32);
		$to_queue.send($topic, $key, $json.to_string()).await;
	)
}

struct EntityRunContext {
	entity_id: u32,
	entity_type: EntityType,
	service_id: String,
	sp_id: String,
	node_id:u32,
	index: usize,
	send_channels: Vec<ChannelStates>,
	wait_receipt_map: HashMap<u64, JsonValue>,
	long_sms_cache: HashMap<String, Vec<Option<JsonValue>>>,
	to_queue: Arc<KafkaMessageProducer>,
	now_conn_num: Arc<AtomicU8>,
	state_change_json: JsonValue,
}

impl Display for EntityRunContext {
	fn fmt(&self, f: &mut Formatter<'_>) -> result::Result<(), Error> {
		write!(f, "id:{},type:{:?},service_id:{},sp_id:{},index:{},now_conn_num:{:?},send_channels:{:?}", self.entity_id, self.entity_type, self.service_id, self.sp_id, self.index, self.now_conn_num, self.send_channels)
	}
}

pub async fn start_entity(mut manager_to_entity_rx: mpsc::
Receiver<JsonValue>, mut from_channel: mpsc::Receiver<JsonValue>, entity_id: u32, service_id: String, sp_id: String, node_id: u32, now_conn_num: Arc<AtomicU8>, entity_type: EntityType) {
	let mut context = EntityRunContext {
		entity_id,
		entity_type,
		service_id,
		sp_id,
		node_id,
		index: 0,
		send_channels: Vec::new(),
		wait_receipt_map: HashMap::new(),
		long_sms_cache: HashMap::new(),
		to_queue: message_sender().clone(),
		now_conn_num,
		state_change_json: json::object! {
						msg_type: "PassageStateChange",
						id: entity_id,
						state: 0
					},
	};
	log::info!("新开始一个entity.{}", context);

	let mut clear_msg_timestamp = chrono::Local::now().timestamp();
	let clear_msg_duration = 86400 * 4;

	let mut clear_timestamp = chrono::Local::now().timestamp();
	let clear_duration = 86400;

	let mut re_send_timestamp = chrono::Local::now().timestamp();
	let re_send_duration = 10;

	loop {
		//一个时间窗口过去,清除发送数据
		if (clear_msg_timestamp + clear_msg_duration) < chrono::Local::now().timestamp() {
			clear_send_sms_cache(&mut context.wait_receipt_map, clear_msg_duration);
			clear_msg_timestamp = chrono::Local::now().timestamp()
		}

		//一个时间窗口过去,清除长短信数据
		if (clear_timestamp + clear_duration) < chrono::Local::now().timestamp() {
			clear_long_sms_cache(&mut context.long_sms_cache, clear_duration);
			clear_timestamp = chrono::Local::now().timestamp()
		}

		//一个时间窗口过去,计算重发
		if (re_send_timestamp + re_send_duration) < chrono::Local::now().timestamp() {
			re_send!(&mut context);
			re_send_timestamp = chrono::Local::now().timestamp()
		}

		tokio::select! {
			from_manager_msg = manager_to_entity_rx.recv() => {
				if !handle_from_manager_rx(from_manager_msg,&mut context).await {
					return;
				}
			}
			from_channel_msg = from_channel.recv() => {
			if !handle_from_channel_rx(from_channel_msg,&mut context).await {
					return;
				}
			}
			_ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
				//这里就是用来当全部都没有动作的时间打开再次进行循环.
			}
		}
	}
}

fn clear_long_sms_cache(cache: &mut HashMap<String, Vec<Option<JsonValue>>>, duration: i64) {
	let now = chrono::Local::now().timestamp();

	cache.retain(|_key, value| {
		//查找是否存在超时的
		value.iter().find(|item| {
			if let Some(json) = item {
				if json[RECEIVE_TIME].as_i64().unwrap_or(0) + duration < now {
					return true;
				}
			}

			return false;
		}).is_none()
	});
}

fn clear_send_sms_cache(cache: &mut HashMap<u64, JsonValue>, duration: i64) {
	let now = chrono::Local::now().timestamp();

	cache.retain(|_key, value| {
		value[RECEIVE_TIME].as_i64().unwrap_or(now) + duration > now
	});
}

///entity处理来自于通道端的消息
async fn handle_from_channel_rx(msg: Option<JsonValue>, context: &mut EntityRunContext) -> bool {
	match msg {
		None => {
			log::info!("发送端已经全部退出。退出。id:{}", context.entity_id);
			return false;
		}
		Some(mut msg) => {
			log::trace!("entity收到channle发来消息。id:{}.msg:{}", context.entity_id, msg);
			msg[RECEIVE_TIME] = chrono::Local::now().timestamp().into();
			msg[ENTITY_ID] = context.entity_id.into();

			match msg[MSG_TYPE_STR].as_str() {
				Some(v) => {
					match (v.into(), msg[WAIT_RECEIPT].as_bool()) {
						(MsgType::SubmitResp, _) => {
							if let Some(mut source) = context.wait_receipt_map.remove(&get_key(&msg)) {
								log::trace!("收到回执..移除缓存:{}", source);

								msg[ACCOUNT_MSG_ID] = source.remove(ACCOUNT_MSG_ID);
								msg[PASSAGE_MSG_ID] = msg.remove(MSG_ID);
							}

							send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT_RESP, "", msg);
						}
						//发送需要等待回执
						(MsgType::Submit, Some(true)) => {
							log::trace!("缓存消息.等待回执..消息:{}", msg);
							insert_into_wait_receipt(&mut context.wait_receipt_map, msg);
						}
						//状态报告需要等待回执
						(MsgType::Deliver, Some(true)) |
						(MsgType::Report, Some(true)) => {
							log::trace!("缓存上行或状态报告消息.等待回执..消息:{}", msg);
							if msg[SEQ_IDS].is_array() && !msg[SEQ_IDS].is_empty() {
								if let Some(seq_id) = msg[SEQ_IDS][0].as_u64() {
									context.wait_receipt_map.insert(seq_id, msg);
								} else {
									log::error!("insert_into_wait_receipt出现错误。SEQ_IDS内为空...json:{}", msg);
								}
							} else {
								log::error!("insert_into_wait_receipt出现错误。当前未取到SEQ_IDS...json:{}", msg);
							}
						}

						(MsgType::ReportResp, _) => {
							log::trace!("收到回执..移除缓存:{}", &get_key(&msg));

							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue!(&context.to_queue, TOPIC_TO_B_REPORT_RESP, "", msg);
						}

						(MsgType::DeliverResp, _) => {
							if let Some(mut item) = context.wait_receipt_map.remove(&get_key(&msg)) {
								log::trace!("收到回执..移除缓存:{}", item);

								msg[ACCOUNT_MSG_ID] = item.remove(MSG_ID);
							}

							send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER_RESP, "", msg);
						}
						(MsgType::Report, Some(false)) |
						(MsgType::Report, None) => {
							if !msg[PASSAGE_MSG_ID].is_empty() {
								msg[MSG_ID] = msg.remove(PASSAGE_MSG_ID);
							}

							send_to_queue!(&context.to_queue, TOPIC_TO_B_REPORT, "", msg);
						}
						//收到上传消息
						(MsgType::Deliver, Some(false)) |
						(MsgType::Deliver, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(mut json) = handle_long_sms(context, msg, total) {
									send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER, "", json);
								}
							} else {
								send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER, "", msg);
							}
						}
						(MsgType::Submit, Some(false)) |
						(MsgType::Submit, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(mut json) = handle_long_sms(context, msg, total) {
									send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT, "", json);
								}
							} else {
								let mut msg_ids = Vec::with_capacity(1);
								msg_ids.push(msg[MSG_ID].as_str().unwrap_or(""));
								msg[MSG_IDS] = msg_ids.into();
								msg.remove(MSG_ID);

								send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT, "", msg);
							}
						}
						(MsgType::Terminate, _) => {
							log::info!("通道关闭操作。msg:{}", msg);

							let id = match msg[ID].as_usize() {
								None => {
									log::error!("未收到关闭请求过来的id。msg:{}..", msg);

									//这里只是退出当前操作
									return true;
								}
								Some(v) => v
							};

							//保留除当前通道外其余通道
							context.send_channels.retain(|item| item.id != id);
							context.now_conn_num.swap(context.send_channels.len() as u8, SeqCst);

							//已经没有连接了.向外发连接断开消息
							if context.send_channels.len() == 0 {
								context.state_change_json[STATE] = 0.into();
								send_entity_state(context).await;
							}
						}
						//有通道连接上了
						(MsgType::Connect, _) => {
							log::info!("有通道连接上了.msg:{}", msg);
							if let Some(ind) = msg["channel_id"].as_u32() {
								let mut save = TEMP_SAVE.write().await;
								if let Some((entity_to_channel_priority_tx, entity_to_channel_common_tx)) = save.remove(&ind) {
									context.send_channels.push(ChannelStates {
										id: ind as usize,
										is_active: true,
										can_write: msg[CAN_WRITE].as_bool().unwrap_or(true),
										entity_to_channel_priority_tx,
										entity_to_channel_common_tx,
									});

									//如果原连接数是0.转成现在有的话.向外发送已经连接消息
									if context.now_conn_num.load(SeqCst) == 0 {
										context.state_change_json[STATE] = 1.into();
										send_entity_state(context).await;
									}

									context.now_conn_num.swap(context.send_channels.len() as u8, SeqCst);
								} else {
									log::error!("收到通道创建消息.但没有在临时存放里面找到它.msg:{}", msg);
								}
							} else {
								log::error!("收到通道创建消息.但没有channel_id字段.msg:{}", msg);
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

	true
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
	//如果是长短信。放多条。重发的时候跳过多余的.
	//如果当前条未收到。准备全部进行重发。
	if json[SEQ_IDS].is_array() && !json[SEQ_IDS].is_empty() &&
		json[MSG_IDS].is_array() && !json[MSG_IDS].is_empty() {
		for i in 0..json[SEQ_IDS].members().len() {
			if let Some(seq_id) = json[SEQ_IDS][i].as_u64() {
				let mut new_one = json.clone();
				new_one[NEED_RE_SEND] = (i == 0).into();
				new_one[ACCOUNT_MSG_ID] = match json[MSG_IDS][i].as_str() {
					Some(msg_id) => msg_id.into(),
					None => {
						log::error!("数据内的msg_ids不足够...json:{}", json);
						return;
					}
				};

				wait_receipt_map.insert(seq_id, new_one);
			} else {
				log::error!("insert_into_wait_receipt出现错误。SEQ_IDS内为空...json:{}", json);
			}
		}
	} else {
		log::error!("insert_into_wait_receipt出现错误。当前未取到SEQ_IDS.或者 msg_ids...json:{}", json);
	}
}

fn handle_long_sms(context: &mut EntityRunContext, msg: JsonValue, total: u8) -> Option<JsonValue> {
	let key = String::from(msg[SRC_ID].as_str().unwrap_or("")).
		add(msg[DEST_ID].as_str().unwrap_or(msg[DEST_IDS][0].as_str().unwrap_or("")));

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
			//当上一次未收完全。可能导致后续另外收消息时候出现此问题。出现此问题。重置数组
			if vec.len() != total as usize {
				*vec = vec![None; total as usize];
			}

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
		let msg_id = match item[MSG_ID].as_str() {
			None => {
				log::error!("从数据里面没有找到.msg_id字段...{}", item);
				""
			}
			Some(msg_id) => msg_id
		};

		msg_ids.push(msg_id.to_owned());
		msg_content.push_str(item[MSG_CONTENT].as_str().unwrap_or(""));
	}

	let mut json = vec.remove(0).unwrap();

	json[MSG_CONTENT] = msg_content.as_str().into();
	json[MSG_IDS] = msg_ids.into();
	json.remove(MSG_ID);
	json.remove(LONG_SMS_TOTAL);
	json.remove(LONG_SMS_NOW_NUMBER);

	context.long_sms_cache.remove(&key);

	Some(json)
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
				Some("send") => {
					send_to_channels(msg, context).await;
				}
				Some("passage.request.state") => {
					log::info!("收到需要状态的消息。id:{}", context.entity_id);

					send_entity_state(context).await;
				}
				Some("close") => {
					log::debug!("开始进行实体的关闭操作。id:{}", context.entity_id);

					for channel in context.send_channels.iter() {
						if let Err(e) = channel.entity_to_channel_priority_tx.send(json::parse(r#"{"msg_type":"Terminate"}"#).unwrap()).await {
							log::error!("发送关闭消息出现异常.e:{}", e);
						}
					}

					context.state_change_json[STATE] = 0.into();
					send_entity_state(context).await;
					return false;
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

	send_msg[ENTITY_ID] = context.entity_id.into();
	send_msg[SP_ID] = context.sp_id.as_str().into();
	send_msg[NODE_ID] = context.node_id.into();
	
	if !context.service_id.is_empty() {
		send_msg[SERVICE_ID] = context.service_id.as_str().into();
	}

	log::debug!("选择一个可用的channel发送.id:{}..现有通道数:{},msg:{}", context.entity_id, context.send_channels.len(),&send_msg);

	let mut failure: Option<(JsonValue, usize)> = None;
	let size = context.send_channels.len();
	let priority = send_msg[IS_PRIORITY].as_bool().unwrap_or(false);
	for _ in 0..size {
		if context.index == usize::MAX {
			context.index = 1;
		}
		context.index += 1;

		match context.send_channels.get(context.index % context.send_channels.len()) {
			Some(select) => {
				if select.can_write {
					log::debug!("收到消息.分channel发送。通道:{}", &select.id);

					if priority {
						if let Err(e) = select.entity_to_channel_priority_tx.send(send_msg).await {
							log::error!("发送正常消息出现异常.e:{}", e);
							failure = Some((e.0, select.id));
						}
					} else {
						if let Err(e) = select.entity_to_channel_common_tx.send(send_msg).await {
							log::error!("发送正常消息出现异常.e:{}", e);
							failure = Some((e.0, select.id));
						}
					}
				} else {
					continue;
				}
			},
			None => {
				continue;
			}
		};

		//当某一个通道端已经退出了，将对应的也退出
		if let Some((json, channel_id)) = failure {
			context.send_channels.retain(|item| item.id != channel_id);
			context.now_conn_num.swap(context.send_channels.len() as u8, SeqCst);
			send_msg = json;
		} else {
			//转发成功的在这里退出
			return;
		}

		failure = None;
	};

	//只有一个通道都没有的时候，才会走到这里.返回错误.同时发连接断开消息
	context.to_queue.send(TOPIC_TO_B_FAILURE, "", send_msg.to_string()).await;
	context.state_change_json[STATE] = 0.into();
}

async fn send_entity_state(context: &mut EntityRunContext) {
	match context.entity_type {
		EntityType::Custom => context.to_queue.send(TOPIC_TO_B_ACCOUNT_STATE_CHANGE, "", context.state_change_json.to_string()).await,
		EntityType::Server => context.to_queue.send(TOPIC_TO_B_PASSAGE_STATE_CHANGE, "", context.state_change_json.to_string()).await,
	}
}
