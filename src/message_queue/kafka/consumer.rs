use std::fmt::{Display, Formatter, Result};

use json::JsonValue;
use log::{error, info, warn,debug};
use rdkafka::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaResult;
use tokio::sync::mpsc;
use rdkafka::Message;


pub struct KafkaMessageConsumer {
	brokers: String,
	group_id: String,
	group_instance_id: String,
	is_connection: bool,
	close_tx: Option<mpsc::Sender<u8>>,
}


impl KafkaMessageConsumer {
	pub fn new(brokers: String,
	           group_id: String,
	           group_instance_id: String,
	) -> KafkaMessageConsumer {
		KafkaMessageConsumer {
			brokers,
			group_id,
			group_instance_id,
			is_connection: false,
			close_tx: None,
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
	pub async fn start(&mut self, topics: &[&str], sender: mpsc::Sender<JsonValue>) -> KafkaResult<()> {
		let consumer: StreamConsumer = ClientConfig::new()
			.set("group.id", &self.group_id)
			.set("group.instance.id", &self.group_instance_id)
			.set("bootstrap.servers", &self.brokers)
			.set("session.timeout.ms", "6000")
			.set("auto.offset.reset", "earliest")
			// Commit automatically every 5 seconds.
			.set("enable.auto.commit", "true")
			.set("auto.commit.interval.ms", "5000")
			.set("enable.auto.offset.store", "false")
			.create()?;

		consumer.subscribe(&topics.to_vec())?;

		self.is_connection = true;
		let (close_tx, mut close_rx) = mpsc::channel::<u8>(1);
		self.close_tx = Some(close_tx);

		loop {
			tokio::select! {
				msg = consumer.recv() => {
					match msg {
						Ok(msg) => {
							let body = match msg.payload_view::<str>() {
								Some(Ok(body)) => {
									debug!("收到消息:msg:{}", body);
									body
								},
								Some(Err(e)) => {
									error!("获得数据出现错误:{}", e);
									continue;
								}
								None => {
									warn!("解析结果为null");
									continue;
								}
							};

							let mut json_msg = match json::parse(body) {
								Ok(json_msg) => json_msg,
								Err(e) => {
									error!("解码json格式出现错误:{}.文本:{}", e, body);
									continue;
								}
							};
							json_msg["topic"] = msg.topic().into();

							if let Err(e) = sender.send(json_msg).await{
								error!("kafka向对端发送数据出现错误,接收端可能已经关闭,e:{}",e);
								return Ok(());
							}
							//设置消息队列偏移
							if let Err(e) = consumer.store_offset(&msg) {
								error!("写kafka偏移出现异常: {}", e);
							}
						}
						Err(e)  => {
							warn!("Kafka error: {}", e);
							//接收异常,过2秒再重新接收。
							tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
						}
					}
				}
				Some(msg) = close_rx.recv() => {
					info!("收到关闭消息。退出当前消息获取。msg:{}",msg);
					return Ok(())
				}
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
			info!("{} {}关闭了。。。", self.brokers, self.group_id);
		} else {
			info!("未初始化。不需要清理");
		}
	}
}
