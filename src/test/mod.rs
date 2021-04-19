#![allow(unused)]


use tokio::time::Instant;
use crate::get_runtime;
use tokio::time;

mod entity;


#[test]
fn test_json() {
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

	std::thread::sleep(time::Duration::from_secs(5));
}



