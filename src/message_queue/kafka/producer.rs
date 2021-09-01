use log::error;
use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use crate::get_runtime;

pub struct KafkaMessageProducer {
	producer: FutureProducer,
}


impl KafkaMessageProducer {
	pub fn new(brokers: &str) -> Self {
		KafkaMessageProducer {
			producer: ClientConfig::new()
				.set("bootstrap.servers", brokers)
				.set("linger.ms", "100")
				.set("metadata.request.timeout.ms", "10000")			
				.set("socket.keepalive.enable", "true")
				.set("queue.buffering.max.ms", "3")
				.set("message.send.max.retries", "10")
				.create()
				.expect("Producer creation error"),
		}
	}

	pub async fn send(&self, topic: &'static str, key: &'static str, msg: String) {
		let producer = self.producer.clone();
		get_runtime().spawn(async move {
			let mut record = FutureRecord::to(topic);
			record = record.key(key).payload(msg.as_str());

			match producer.send(record, Timeout::Never).await {
				Ok(_) => {
					log::info!("向消息队列发送消息.topic:{},key:{},msg:{}", topic, key, msg);
				}
				Err((error, message)) => {
					error!("kafka发送消息失败:e:{},topic:{}.message:{:?}", error, topic, message);
				}
			}
		});
	}
}
