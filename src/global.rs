use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use json::JsonValue;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{ RwLock};

use crate::entity::Entity;
use crate::message_queue::KafkaMessageProducer;

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


static mut SEQ_ID: Option<Arc<AtomicU32>> = None;

///全局唯一..Sequence_Id
pub fn get_sequence_id() -> Arc<AtomicU32> {
	unsafe {
		SEQ_ID.get_or_insert_with(|| {
			println!("初始化全局唯一ID");
			Arc::new(AtomicU32::new(rand::random()))
		}).clone()
	}
}

static mut ENTITY_MAP: Option<Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>>> = None;

pub fn get_entity_map() -> Arc<RwLock<HashMap<u32, Arc<dyn Entity>>>> {
	unsafe {
		ENTITY_MAP.get_or_insert_with(|| {
			Arc::new(RwLock::new(HashMap::new()))
		}).clone()
	}
}

pub async fn get_entity(id: &u32) -> Option<Arc<dyn Entity>> {
	let en = &get_entity_map();
	let read = en.read().await;
	return match read.get(id){
		Some(n) => Some(n.clone()),
		None => None
	}
}
