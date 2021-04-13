#![allow(unused)]

use chrono::{DateTime, Local, Datelike, Timelike};
use crate::protocol::Protocol::CMPP48;
use crate::protocol::{Cmpp48, ProtocolImpl};
use bytes::{BytesMut, Buf};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};

mod entity;


#[test]
fn test_json() {

	let d = 0 as usize;
	let b = 21 as usize;

// tx.
	// let semaphore = (semaphore::Semaphore::new(buffer), buffer);
	// let (tx, rx) = chan::channel(semaphore);

	//
	// let tx = Sender::new(tx);
	// let rx = Receiver::new(rx);
	//
	// let (tx,rx) = mpsc::channel(0xFFFFFFFF);
	// tx.chan.for_each(|x| println!("{}",x));
}


