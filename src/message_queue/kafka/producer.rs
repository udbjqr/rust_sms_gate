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
			producer:
			ClientConfig::new()
				.set("bootstrap.servers", brokers)
				.set("message.timeout.ms", "5000")
				.create()
				.expect("Producer creation error"),
		}
	}

	pub async fn send(&self, topic: &'static str, key: &'static str, msg: String) {
		let producer = self.producer.clone();
		get_runtime().spawn(async move{
			let mut record = FutureRecord::to(topic);
			record = record.key(key).payload(msg.as_str());

			match producer.send(record, Timeout::Never).await {
				Ok(_) => {
					log::trace!("向消息队列发送消息.topic:{},key:{},msg:{}", topic, key, msg);
				}
				Err((error, message)) => {
					error!("kafka发送消息失败:e:{},topic:{}.message:{:?}", error, topic, message);
				}
			}
		});
	}
}
