use tokio::sync::{mpsc, RwLock};
use json::JsonValue;
use std::sync::Arc;
use crate::entity::{ChannelStates, EntityManager};
use std::collections::HashMap;

pub async fn start_entity(mut manager_to_entity_rx: mpsc::Receiver<JsonValue>, mut from_channel: mpsc::Receiver<JsonValue>, channels: Arc<RwLock<Vec<ChannelStates>>>) {
	let mut channels_map: HashMap<usize, (mpsc::Sender<JsonValue>, mpsc::Sender<JsonValue>)> = HashMap::new();
	loop {
		tokio::select! {
			from_manager_msg = manager_to_entity_rx.recv() => {
				handle_from_manager_rx(from_manager_msg,&channels,&mut channels_map).await;
			}
			from_channel_msg = from_channel.recv() => {
				handle_from_channel_rx(from_channel_msg,&channels,&mut channels_map).await;
			}
		}
	}
}

///entity处理来自于通道端的消息
async fn handle_from_channel_rx(msg: Option<JsonValue>, channels: &Arc<RwLock<Vec<ChannelStates>>>, channels_map: &mut HashMap<usize, (mpsc::Sender<JsonValue>, mpsc::Sender<JsonValue>)>) {
	match msg {
		None => {
			log::info!("发送端已经全部退出。退出。");
			return;
		}
		Some(msg) => {
			match msg["msg_type"].as_str() {
				Some("close") => {
					log::debug!("通道关闭操作。");

					let id = match msg["id"].as_usize() {
						None => {
							log::error!("未收到关闭请求过来的id。msg:{}", msg);
							return;
						}
						Some(v) => v
					};

					channels_map.remove(&id);
					let mut channels = channels.write().await;
					if let Some(mut item) = channels.get_mut(id) {
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

///entity处理来自于管理器端的消息
async fn handle_from_manager_rx(msg: Option<JsonValue>, channels: &Arc<RwLock<Vec<ChannelStates>>>, channels_map: &mut HashMap<usize, (mpsc::Sender<JsonValue>, mpsc::Sender<JsonValue>)>) {
	match msg {
		None => {
			log::info!("发送端已经退出。直接退出。");
			return;
		}
		Some(msg) => {
			match msg["msg_type"].as_str() {
				Some("create") => {
					let entity_id = msg["entity_id"].as_u32().unwrap();
					let channel_id = msg["channel_id"].as_usize().unwrap();
					let manager = EntityManager::get_entity_manager();
					let entitys = manager.entitys.read().await;

					if let Some(entity) = entitys.get(&entity_id) {
						let entity = entity.get_channels();
						let entity = entity.read().await;
						if let Some(channel) = entity.get(channel_id) {
							channels_map.insert(channel_id,
							                    (channel.entity_to_channel_priority_tx.as_ref().unwrap().clone(),
							                     channel.entity_to_channel_common_tx.as_ref().unwrap().clone()));
						} else {
							log::error!("没有找到对应的channel..msg:{}", msg);
						}
					} else {
						log::error!("没有找到对应的entity..msg:{}", msg);
					}
				}
				Some("close") => {
					log::debug!("开始进行实体的关闭操作。");

					let mut items = channels.write().await;
					for i in 0..items.len() {
						if let Some(sender) = channels_map.get(&i) {
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
