use std::io;

use bytes::BytesMut;
use json::JsonValue;
use std::io::Error;

pub mod cmpp;

pub trait Protocol {
	fn login_msg(&self, sp_id: &str, password: &str, version: u8, seq: u32) -> Result<BytesMut, io::Error>;

	fn decode_read_message(&self, buf: &mut BytesMut) -> Result<JsonValue, Error> ;
}

#[derive(Debug)]
pub enum ProtocolType{
	CMPP,
	SGIP,
	SMGP
}
