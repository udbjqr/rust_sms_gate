#![allow(unused)]

use std::sync::Arc;

use crate::entity::{CustomEntity, Entity, EntityManager};
use crate::get_runtime;
use crate::global::{ISMG_ID, get_sequence_id, message_sender};
use json::JsonValue;
use std::time::SystemTime;
use chrono::{Local, Datelike, Timelike};
use std::str::FromStr;
use std::collections::HashMap;
use std::net::{SocketAddr, Ipv4Addr};


#[test]
fn test_json_array() {
	let json = json::object! {arr:["123","456","789"]};

	println!("{}", json["arr"][0].as_str().unwrap_or("没有东西呀"));
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
	result = result << 16 | (get_sequence_id(1) & 0xff) as u64;

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

#[test]
fn test_addr() {
	let d: Ipv4Addr = "55.3.2.3".parse().unwrap();
	let d_v = d.octets();
	let s = "128.3.3.3,128.3.3.3,0.3.2.0,128.34.2.3"; //,128.3.3.3,55.3.2.3,128.34.2.3,2.3.2.3
	let mut v = s.split(",");
	let mut found = true;

	dbg!(s.split(",").find(|addr| {
		if let Ok(addr) = addr.parse::<Ipv4Addr>() {
			let v = addr.octets();
			let mut found = true;
			for ind in 0..v.len() {
				found = v[ind] == d_v[ind] || v[ind] == 0;
				if !found { return false; }
			}

			found
		} else {
			false
		}
	}).is_some());


	//
	// for i in v{
	//
	// 	found = true;
	// 	let addr: Ipv4Addr = i.parse().unwrap();
	//
	// 	let addr_v = addr.octets();
	//
	// 	for i in 0..4 {
	// 		dbg!(addr_v[i],d_v[i],!(addr_v[i] == d_v[i] || addr_v[i] == 0));
	// 		if !(addr_v[i] == d_v[i] || addr_v[i] == 0) {
	// 			found = false;
	// 			break;
	// 		}
	// 	}
	//
	// 	if found{
	// 		break;
	// 	}
	// }

	// dbg!("找到了相应的数据。",found);

	//
	// let addr: Ipv4Addr = "128.3.2.3".parse().unwrap();
	// let d: Ipv4Addr = "127.0.0.1".parse().unwrap();
	//
	// let addr_v = addr.octets();
	// let d_v = d.octets();
	//
	// let mut found = true;
	// for i in 0..4 {
	// 	if addr_v[i] != d_v[i] && addr_v[i] != 0 {
	// 		found = false;
	// 	}
	// }
	//
	// dbg!(found);
	// if addr.is_unspecified() {
	// 	println!("这是0.0.0.0");
	// }
	//
	// dbg!(addr);
}
