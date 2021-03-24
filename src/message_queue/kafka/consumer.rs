use std::fmt::{Display, Formatter, Result};

use log::{error, info, warn};
use rdkafka::{ClientConfig};
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaResult;
use tokio::sync::{mpsc};

use crate::get_runtime;
use crate::message_queue::QueueType;
use crate::message_queue::queue_handle::handle_queue_msg;

pub struct KafkaMessageConsumer {
	brokers: String,
	group_id: String,
	topic: String,
	is_connection: bool,
	close_tx: Option<mpsc::Sender<u8>>,
	queue_type:QueueType
}


impl KafkaMessageConsumer {
	pub fn new(brokers: String,
	           group_id: String,
	           topic: String,
	           queue_type:QueueType,
	) -> KafkaMessageConsumer {
		KafkaMessageConsumer {
			brokers,
			group_id,
			topic,
			is_connection: false,
			close_tx: None,
			queue_type
		}
	}

	pub async fn close(&mut self) {
		if let Some(tx) = &self.close_tx {
			if let Err(e) = tx.clone().send(0).await {
				warn!("发送关闭出现一个问题:{}", e)
			}
		}
	}

	///准备启动
	pub async fn start(&mut self) -> KafkaResult<()> {
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
		let (close_tx, _close_rx) = mpsc::channel::<u8>(1);
		self.close_tx = Some(close_tx);

		loop {
			tokio::select! {
				Ok(msg) = consumer.recv() => {
					//这里是用协程,就必须将消息转换一份。msg.detach()进行了内存操作。把数据复制了一份。
					//转换消息的开销和使用协程带来的运算优势到底哪一个更好?
					get_runtime().spawn(handle_queue_msg(msg.detach(),self.queue_type));

					if let Err(e) = consumer.store_offset(&msg) {
						error!("写kafka偏移出现异常: {}", e);
					}
				}
				// Err(e) = consumer.recv() => {
				// 	warn!("Kafka error: {}", e);
				// 	//接收异常,过2秒再重新接收。
				// 	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
				// }
				// Some(msg) = close_rx.recv() => {
				// 	info!("收到关闭消息。退出当前消息获取。msg:{}",msg);
				// 	return Ok(())
				// }
			}
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
