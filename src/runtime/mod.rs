use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::{Builder, Runtime};

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

