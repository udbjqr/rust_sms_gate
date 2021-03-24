pub use self::kafka::consumer::KafkaMessageConsumer;
pub use self::kafka::producer::KafkaMessageProducer;

///消息队列的内容
pub mod kafka;
pub mod queue_handle;

#[derive(Copy, Clone)]
pub enum QueueType {
	SendMessage = 1,
	CloseEntity = 2,
}
