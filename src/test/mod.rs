#![allow(unused)]

use chrono::{DateTime, Local, Datelike, Timelike};
mod entity;


#[test]
fn test_json() {
	simple_logger::SimpleLogger::new().init().unwrap();
	// let json = json::parse(r#"{"id":23,"login_name":10682203,"address":"47.114.180.42:7890","password":123456,"version":"48","name":"测试通道"}"#).unwrap();

	log::error!("12341234123412341");
}

