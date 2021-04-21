use tokio::sync::{mpsc};
use json::JsonValue;
use std::sync::Arc;
use crate::entity::{ChannelStates, EntityType};
use std::collections::HashMap;
use crate::protocol::names::{MANAGER_TYPE, MSG_TYPE_STR, ID, WAIT_RECEIPT, SEQ_ID, LONG_SMS_TOTAL, SRC_ID, DEST_ID, DEST_IDS, LONG_SMS_NOW_NUMBER, MSG_ID, MSG_CONTENT, MSG_IDS, RECEIVE_TIME, ENTITY_ID, SEQ_IDS, IS_PRIORITY, SP_ID, SERVICE_ID, STATE, DURATION};
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

			if receive_time + duration < now {
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
		$to_queue.send($topic, $key, $json.to_string()).await;
	)
}

struct EntityRunContext {
	entity_id: u32,
	entity_type: EntityType,
	service_id: String,
	sp_id: String,
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
Receiver<JsonValue>, mut from_channel: mpsc::Receiver<JsonValue>, entity_id: u32, service_id: String, sp_id: String,
                          now_conn_num: Arc<AtomicU8>, entity_type: EntityType) {
	let mut context = EntityRunContext {
		entity_id,
		entity_type,
		service_id,
		sp_id,
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
	log::trace!("新开始一个entity.{}", context);

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
				if !handle_from_manager_rx(from_manager_msg,&mut context).await{
					return;
				}
			}
			from_channel_msg = from_channel.recv() => {
			if !handle_from_channel_rx(from_channel_msg,&mut context).await{
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
		//里面没有一个超时了.返回false
		value.iter().find(|item| {
			let mut found = false;
			if let Some(json) = item {
				if json[RECEIVE_TIME].as_i64().unwrap_or(0) + duration <= now {
					found = true;
				}
			}

			found
		}).is_none()
	});
}

fn clear_send_sms_cache(cache: &mut HashMap<u64, JsonValue>, duration: i64) {
	let now = chrono::Local::now().timestamp();

	cache.retain(|_key, value| {
		value[RECEIVE_TIME].as_i64().unwrap_or(now) + duration <= now
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
			log::trace!("收到通道发来消息。id:{}.msg:{}", context.entity_id, msg);
			msg[RECEIVE_TIME] = chrono::Local::now().timestamp().into();
			msg[ENTITY_ID] = context.entity_id.into();

			match msg[MSG_TYPE_STR].as_str() {
				Some(v) => {
					match (v.into(), msg[WAIT_RECEIPT].as_bool()) {
						//需要等待回执
						(MsgType::Deliver, Some(true)) |
						(MsgType::Submit, Some(true)) |
						(MsgType::Report, Some(true)) => {
							log::trace!("缓存一个消息.等待回执..消息:{}", msg);
							insert_into_wait_receipt(&mut context.wait_receipt_map, msg);
						}
						(MsgType::ReportResp, _) => {
							log::trace!("收到回执..移除缓存:{}", &get_key(&msg));
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue!(&context.to_queue, TOPIC_TO_B_REPORT_RESP, "", msg);
						}
						(MsgType::DeliverResp, _) => {
							log::trace!("收到回执..移除缓存:{}", &get_key(&msg));
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER_RESP, "", msg);
						}
						(MsgType::SubmitResp, _) => {
							log::trace!("收到回执..移除缓存:{}", &get_key(&msg));
							context.wait_receipt_map.remove(&get_key(&msg));
							send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT_RESP, "", msg);
						}
						//收到上传消息
						(MsgType::Report, Some(false)) |
						(MsgType::Report, None) => {
							send_to_queue!(&context.to_queue, TOPIC_TO_B_REPORT, "", msg);
						}
						//收到上传消息
						(MsgType::Deliver, Some(false)) |
						(MsgType::Deliver, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(json) = handle_long_sms(context, msg, total) {
									send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER, "", json);
								}
							} else {
								send_to_queue!(&context.to_queue, TOPIC_TO_B_DELIVER, "", msg);
							}
						}
						(MsgType::Submit, Some(false)) |
						(MsgType::Submit, None) => {
							if let Some(total) = msg[LONG_SMS_TOTAL].as_u8() {
								if let Some(json) = handle_long_sms(context, msg, total) {
									send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT, "", json);
								}
							} else {
								send_to_queue!(&context.to_queue, TOPIC_TO_B_SUBMIT, "", msg);
							}
						}
						(MsgType::Terminate, _) => {
							log::debug!("通道关闭操作。msg:{}", msg);

							let id = match msg[ID].as_usize() {
								None => {
									log::error!("未收到关闭请求过来的id。msg:{}", msg);
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
							log::trace!("收到通道发过来的连接消息.msg:{}", msg);
							if let Some(ind) = msg["channel_id"].as_u32() {
								let mut save = TEMP_SAVE.write().await;
								if let Some((entity_to_channel_priority_tx, entity_to_channel_common_tx)) = save.remove(&ind) {
									context.send_channels.push(ChannelStates {
										id: ind as usize,
										is_active: true,
										can_write: true,
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
					log::trace!("收到需要状态的消息。id:{}", context.entity_id);

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
	if !context.service_id.is_empty() {
		send_msg[SERVICE_ID] = context.service_id.as_str().into();
	}

	log::trace!("选择一个可用的channel发送.id:{}..现有通道数:{}", context.entity_id, context.send_channels.len());

	while !context.send_channels.is_empty() {
		let mut failure = None;
		if context.index == usize::MAX {
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

						//一个通道都没有的时候.返回错误.同时发连接断开消息
						context.to_queue.send(TOPIC_TO_B_FAILURE, "", send_msg.to_string()).await;
						context.state_change_json[STATE] = 0.into();
						// context.to_queue.send(TOPIC_TO_B_PASSAGE_STATE_CHANGE, "", context.state_change_json.to_string().as_str()).await;
						send_entity_state(context).await;
						return;
					}
				}

				Some(se) => se
			};
			log::debug!("收到消息.分channel发送。通道:{}.msg:{}", &select.id, &send_msg);

			if send_msg[IS_PRIORITY].as_bool().unwrap_or(false) {
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
		}

		if let Some((json, channel_id)) = failure {
			log::trace!("选择通道失败.id:{},channel_id:{}", context.entity_id, channel_id);
			// 对端可能已经关闭.当前的通道已经不可用,删除通道并且重选.
			context.send_channels.retain(|item| item.id != channel_id);
			context.now_conn_num.swap(context.send_channels.len() as u8, SeqCst);
			send_msg = json;
		} else {
			return;
		}
	}
}

async fn send_entity_state(context: &mut EntityRunContext) {
	match context.entity_type {
		EntityType::Custom => context.to_queue.send(TOPIC_TO_B_ACCOUNT_STATE_CHANGE, "", context.state_change_json.to_string()).await,
		EntityType::Server => context.to_queue.send(TOPIC_TO_B_PASSAGE_STATE_CHANGE, "", context.state_change_json.to_string()).await,
	}
}
