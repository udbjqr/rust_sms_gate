#![allow(unused)]


use tokio::time::Duration;
use tokio::time::sleep;
use sms_gate::get_runtime;
use tokio::time;

///这里现在启动解码器的协议编码解码部分
fn main() {
	// time::Duration::from_millis();
	let d = get_runtime().spawn(async move {
		loop {
			tokio::select! {
				biased;
				_ = time::sleep(time::Duration::from_secs(2)) =>{
					println!("one");
				}
				_ = time::sleep(time::Duration::from_secs(1)) =>{
					println!("two");
				}
			}
		}
	});

	std::thread::sleep(time::Duration::from_secs(15));
}

