use std::fmt::{Display, Formatter, Result};

use log::{debug, error, info, warn};
use rdkafka::{ClientConfig, Message};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::BorrowedMessage;
use tokio::sync::{mpsc::{self, Sender}};

use crate::get_runtime;

pub struct KafkaMessageConsumer {
	brokers: String,
	group_id: String,
	topic: String,
	is_connection: bool,
	close_tx: Option<Sender<u8>>,
}


impl KafkaMessageConsumer {
	pub fn new(brokers: String,
	           group_id: String,
	           topic: String) -> KafkaMessageConsumer {
		KafkaMessageConsumer {
			brokers,
			group_id,
			topic,
			is_connection: false,
			close_tx: None,
		}
	}

	pub fn close(&mut self) {
		if self.close_tx.is_some() {
			let tx = self.close_tx.clone();
			get_runtime().spawn(async move {
				if let Err(e) = tx.unwrap().send(0).await {
					warn!("发送关闭出现一个问题:{}", e)
				}
			});
		}
	}

	pub fn start(&mut self, tx: Sender<String>) -> KafkaResult<()> {
		let consumer: StreamConsumer = ClientConfig::new()
			.set("group.id", &self.group_id)
			.set("bootstrap.servers", &self.brokers)
			.set("session.timeout.ms", "6000")
			.set("auto.offset.reset", "earliest")
			// Commit automatically every 5 seconds.
			.set("enable.auto.commit", "true")
			.set("auto.commit.interval.ms", "5000")
			.set("enable.auto.offset.store", "false")
			.create()?;

		consumer.subscribe(&[&self.topic])?;

		self.is_connection = true;
		let (close_tx, mut close_rx) = mpsc::channel::<u8>(1);

		let d = KafkaMessageConsumer {
			brokers: self.brokers.clone(),
			group_id: self.group_id.clone(),
			topic: self.topic.clone(),
			is_connection: false,
			close_tx: None,
		};

		self.close_tx = Some(close_tx);

		get_runtime().spawn(async move {
			loop {
				tokio::select! {
					Ok(msg) = consumer.recv() => {
						d.handle_get_data(&tx, &consumer, &msg).await;
					}
					Err(e) = consumer.recv() => {
						warn!("Kafka error: {}", e);
						return;
					}
					Some(msg) = close_rx.recv() =>{
						info!("收到关闭。退出操作。msg:{}",msg);
						return;
					}
				}
			}
		});

		Ok(())
	}

	async fn handle_get_data(&self, tx: &Sender<String>, consumer: &StreamConsumer, m: &BorrowedMessage<'_>) {
		match m.payload_view::<str>() {
			Some(Ok(msg)) => {
				debug!("收到消息:{}", msg);

				if let Err(e) = tx.send(msg.to_owned()).await {
					warn!("消费者:{}出现发送错误: {}", self, e);
				}
			}
			Some(Err(e)) => {
				error!("消费者:{}获得数据出现错误:{}", &self, e);
			}
			None => {
				warn!("消费者:{}解析结果为null", self);
			}
		}

		if let Err(e) = consumer.store_offset(&m) {
			error!("消费者:{}写kafka偏移出现异常: {}", &self, e);
		}
	}
}


impl Display for KafkaMessageConsumer {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		write!(f, "KafkaMessage{{}}")
	}
}

impl Drop for KafkaMessageConsumer {
	fn drop(&mut self) {
		if self.is_connection {
			info!("{} {} {}关闭了。。。", self.brokers, self.group_id, self.topic);
		} else {
			info!("未初始化。不需要清理");
		}
	}
}
