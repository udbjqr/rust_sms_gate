#![allow(unused)]
use std::sync::Arc;

use crate::entity::{CustomEntity, Entity, get_entity, get_entity_map, insert_entity};
use crate::get_runtime;

#[tokio::test(flavor = "multi_thread", worker_threads = 10)]
async fn test_entity_manager() {
	println!("准备:");
	get_runtime().spawn(async move {
		let entity = CustomEntity::new(10, "abc".to_string(), "abcdefg".to_string(), "12345678".to_string(), vec!["127.0.0.1".to_string()], 200, 200, 2);

		insert_entity(entity.get_id(), Arc::new(entity)).await;

		let entity = CustomEntity::new(20, "abc".to_string(), "上面的".to_string(), "765432".to_string(), vec!["127.0.0.1".to_string()], 200, 200, 2);
		insert_entity(entity.get_id(), Arc::new(entity)).await;

		// let d = get_entity(10).await;

		// println!("放入Map以后的数。{:?}", d)
	});

	get_runtime().spawn(async move {
		tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

		let entity = CustomEntity::new(20, "abc".to_string(), "第二个".to_string(), "765432".to_string(), vec!["127.0.0.1".to_string()], 200, 200, 2);

		insert_entity(entity.get_id(), Arc::new(entity)).await;

		let map = get_entity_map();
		let map = map.read().await;
		println!("{}", map.len());

		let entity = get_entity(20).await;
		println!("放入Map以后的数。{:?}", entity);
	});

	tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
}

