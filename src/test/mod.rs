#![allow(unused)]

use chrono::{DateTime, Local, Datelike, Timelike};
use crate::protocol::Protocol::CMPP48;
use crate::protocol::{Cmpp48, ProtocolImpl};
use bytes::{BytesMut, Buf};
use futures::FutureExt;

mod entity;


#[test]
fn test_json() {
}


