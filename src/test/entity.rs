#![allow(unused)]

use std::sync::Arc;

use crate::entity::{CustomEntity, Entity, EntityManager};
use crate::get_runtime;
use crate::global::{ISMG_ID, get_sequence_id};
use json::JsonValue;
use std::time::SystemTime;
use chrono::{Local, Datelike, Timelike};
use std::str::FromStr;

#[test]
fn test_json_array() {
	let json = json::object! {arr:["123","456","789"]};

	println!("{}",json["arr"][0].as_str().unwrap_or("没有东西呀"));
}


#[test]
fn test_div() {
	let date = Local::now();
	dbg!(date.month(),date.day(),date.hour(),date.minute(),date.second(),*ISMG_ID,get_sequence_id(1)& 0xff);
	let mut result: u64 = date.month() as u64;
	result = result << 5 | (date.day() & 0x1F) as u64;
	result = result << 5 | (date.hour() & 0x1F) as u64;
	result = result << 6 | (date.minute() & 0x3F) as u64;
	result = result << 6 | (date.second() & 0x3F) as u64;
	result = result << 22 | *ISMG_ID as u64;
	result = result << 16 | (get_sequence_id(1)& 0xff) as u64;

	println!("result:{:X}", result);
}

//
//
// #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
// async fn test_manager() {
// 	println!("准备:");
//
// 	let manager = EntityManager::new();
// 	manager.init();
//
// 	tokio::time::sleep(tokio::time::Duration::from_secs(9994)).await;
// }
//
// #[test]
// fn test_str(){
// 	let d:Vec<String> = "abc,abc2,abc3".split(",").map(|s| s.to_string()).collect();
// 	dbg!(d);
// }
//
// #[tokio::test(flavor = "multi_thread", worker_threads = 10)]
// async fn test_manager() {
// 	println!("准备:");
//
// 	let manager = EntityManager::new();
// 	manager.init();
//
// 	tokio::time::sleep(tokio::time::Duration::from_secs(9994)).await;
// }
