#![allow(unused)]


use tokio::time::Instant;
use crate::get_runtime;
use tokio::time;
use crate::protocol::Protocol::SMGP;
use crate::protocol::smgp::Smgp;
use crate::protocol::ProtocolImpl;
use crate::protocol::implements::get_time;

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


#[test]
fn test_smgpconnect() {
	let c = Smgp::new();
	let mut json = json::object! {
		loginName:"103996",
		password:"123456",
		protocolVersion: 48,
		msg_type: "Connect"
	};

	let e = c.encode_connect(&mut json).unwrap();

	println!("{:x}",e);
}

#[test]
fn test_test() {
	println!("{}",get_time());
}

