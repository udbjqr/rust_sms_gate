#![allow(unused)]


use tokio::time::Duration;
use tokio::time::sleep;
use sms_gate::get_runtime;

///这里现在启动解码器的协议编码解码部分
fn main() {
	//设置日志启动
	simple_logger::SimpleLogger::new().init().unwrap();

	get_runtime().spawn(async move{
		loop {
			// let sleep = time::sleep(Duration::from_millis(1000));
			sleep(Duration::from_millis(1000)).await;
			println!("operation timed out");
			tokio::select! {
            _ = sleep(Duration::from_millis(1000)) => {
                println!("operation timed out");
            }
        }
		}
	});

	std::thread::park();
}

