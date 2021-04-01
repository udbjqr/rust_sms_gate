#![allow(unused)]
use std::sync::Arc;

use crate::entity::{CustomEntity, Entity, EntityManager};
use crate::get_runtime;
use crate::global::load_config_file;
use json::JsonValue;



#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
async fn test_manager() {
	println!("准备:");

	let manager = EntityManager::new();
	manager.init();

	tokio::time::sleep(tokio::time::Duration::from_secs(9994)).await;
}

#[test]
fn test_str(){
	let d:Vec<String> = "abc,abc2,abc3".split(",").map(|s| s.to_string()).collect();
	dbg!(d);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
async fn test_manager() {
	println!("准备:");

	let manager = EntityManager::new();
	manager.init();

	tokio::time::sleep(tokio::time::Duration::from_secs(9994)).await;
}
