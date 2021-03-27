
use sms_gate::entity::ServersManager;

fn main() {
	simple_logger::SimpleLogger::new().init().unwrap();
	ServersManager::start().unwrap();


	// let mut con = KafkaMessageConsumer::new("192.168.101.99:9092".to_owned(),
	//                                         "group".to_owned(),
	//                                         "test22".to_owned(),
	//                                         QueueType::SendMessage, );
	//
	//
	// get_runtime().spawn(async move {
	// 	if let Err(e) = con.start().await {
	// 		error!("启动出现了异常:{}", e);
	// 	}
	// });

	std::thread::park();
}
