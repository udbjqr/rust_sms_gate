#![allow(unused)]


use tokio::time::Duration;
use tokio::time::sleep;
use sms_gate::get_runtime;

///这里现在启动解码器的协议编码解码部分
fn main() {
	//设置日志启动

	log::debug!("这是debug;");
	log::trace!("这是trace;");
	log::info!("这是info;");
	log::warn!("这是warn;");
	log::error!("这是error;");
}

