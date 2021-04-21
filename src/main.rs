use sms_gate::entity::{EntityManager, ServersManager};
use sms_gate::get_runtime;
use sms_gate::global::{message_sender, TOPIC_TO_B_LOWER_COMPUTER_INIT};

fn main() {
	//设置日志启动
	log4rs::init_file("config/log.yaml", Default::default()).unwrap();

	//使用enter 加载运行时。必须需要let _guard 要不没有生命周期。
	let runtime = get_runtime();
	let _guard = runtime.enter();

	//通过这句启动一下。
	EntityManager::get_entity_manager();

	if let Err(e) = ServersManager::start() {
		log::error!("启动服务等待接收异常。退出。e:{}", e);
		return;
	}

	get_runtime().spawn(async move {
		message_sender().send(TOPIC_TO_B_LOWER_COMPUTER_INIT, "", "{}".to_string()).await;
	});

	std::thread::park();
}
