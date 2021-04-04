use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use json::JsonValue;
use lazy_static::lazy_static;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::RwLock;

use crate::message_queue::KafkaMessageProducer;

pub static TOPIC_ACCOUNT_MODIFY: &'static str = "account.modify";
pub static TOPIC_ACCOUNT_ADD: &'static str = "account.add";
pub static TOPIC_ACCOUNT_REMOVE: &'static str = "account.remove";
pub static TOPIC_PASSAGE_MODIFY: &'static str = "passage.modify";
pub static TOPIC_PASSAGE_ADD: &'static str = "passage.add";
pub static TOPIC_PASSAGE_REMOVE: &'static str = "passage.remove";


lazy_static! {
	static ref CONFIG: RwLock<JsonValue> = RwLock::new(load_config_file("config/setting.json"));
	static ref SEQUENCE: AtomicU32 = AtomicU32::new(rand::random());
	pub static ref FILL_ZERO: Vec<u8> = vec![0;200];

	// static ref SERVERS_CONFIG: RwLock<JsonValue> = RwLock::new(load_config_file("smsServer.json"));
}

static mut MESSAGE_SENDER: Option<Arc<KafkaMessageProducer>> = None;

pub fn message_sender() -> Arc<KafkaMessageProducer> {
	unsafe {
		MESSAGE_SENDER.get_or_insert_with(|| {
			let config = load_config_file("config/message_receiver.json");

			Arc::new(KafkaMessageProducer::new(config["bootstrap"]["servers"].as_str().unwrap()))
		}).clone()
	}
}

pub fn load_config_file(file_name: &str) -> JsonValue {
	let file_text = fs::read_to_string(file_name).unwrap();
	let json = json::parse(file_text.as_str()).unwrap();

	json
}

pub async fn get_config_or<T>(name: &str, default: T) -> T
	where T: FromStr {
	let config = CONFIG.read().await;

	let result: T = config[name].dump().parse().unwrap_or(default);

	result
}

static mut RUNTIME: Option<Arc<Runtime>> = None;

pub fn get_runtime() -> Arc<Runtime> {
	unsafe {
		RUNTIME.get_or_insert_with(|| {
			let thread_num = num_cpus::get();

			println!("init pool ~~..CPU数量:{}", thread_num);
			Arc::new(Builder::new_multi_thread()
				.worker_threads(thread_num)
				.thread_name_fn(|| {
					static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
					let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
					format!("工作{}", id)
				})
				.thread_stack_size(2 * 1024 * 1024)
				.enable_all()
				.on_thread_start(|| {
					println!("线程`{}`启动。。。", std::thread::current().name().unwrap())
				})
				.build()
				.unwrap())
		}).clone()
	}
}


static mut QUEUE_PRODUCERS: Option<Arc<RwLock<HashMap<String, KafkaMessageProducer>>>> = None;

pub fn get_producers() -> Arc<RwLock<HashMap<String, KafkaMessageProducer>>> {
	unsafe {
		QUEUE_PRODUCERS.get_or_insert_with(|| {
			println!("初始化发送Map");
			Arc::new(RwLock::new(HashMap::new()))
		}).clone()
	}
}


static mut WAIT_RES: Option<Arc<RwLock<HashMap<u32, JsonValue>>>> = None;

///等待对端收到响应的Map
pub fn get_wait_res() -> Arc<RwLock<HashMap<u32, JsonValue>>> {
	unsafe {
		WAIT_RES.get_or_insert_with(|| {
			println!("初始化等待响应Map");
			Arc::new(RwLock::new(HashMap::with_capacity(0xffffff)))
		}).clone()
	}
}


///全局唯一..Sequence_Id.
/// 参数为步进多少。
pub fn get_sequence_id(mut step: u32) -> u32 {
	if step == 0 {
		step = 1;
	}
	SEQUENCE.fetch_add(step, Ordering::Relaxed)
}