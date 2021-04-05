#![allow(unused)]

use bytes::BytesMut;
use json::JsonValue;
use tokio::io;
use tokio::io::Error;
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::protocol::{Protocol, SmsStatus};
use crate::protocol::msg_type::MsgType;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum SgipType {
	SgipConnect = 0x00000001,
	SgipConnectResp = 0x80000001,
	SgipTerminate = 0x00000002,
	SgipTerminateResp = 0x80000002,
	SgipSubmit = 0x00000004,
	SgipSubmitResp = 0x80000004,
	SgipDeliver = 0x00000005,
	SgipDeliverResp = 0x80000005,
	SgipQuery = 0x00000006,
	SgipQueryResp = 0x80000006,
	SgipCancel = 0x00000007,
	SgipCancelResp = 0x80000007,
	SgipActiveTest = 0x00000008,
	SgipActiveTestResp = 0x80000008,
	SgipFwd = 0x00000009,
	SgipFwdResp = 0x80000009,
	SgipMtRoute = 0x00000010,
	SgipMtRouteResp = 0x80000010,
	SgipMoRoute = 0x00000011,
	SgipMoRouteResp = 0x80000011,
	SgipGetMtRoute = 0x00000012,
	SgipGetMtRouteResp = 0x80000012,
	SgipMtRouteUpdate = 0x00000013,
	SgipMtRouteUpdateResp = 0x80000013,
	SgipMoRouteUpdate = 0x00000014,
	SgipMoRouteUpdateResp = 0x80000014,
	SgipPushMtRouteUpdate = 0x00000015,
	SgipPushMtRouteUpdateResp = 0x80000015,
	SgipPushMoRouteUpdate = 0x00000016,
	SgipPushMoRouteUpdateResp = 0x80000016,
	SgipGetMoRoute = 0x00000017,
	SgipGetMoRouteResp = 0x80000017,
	UnKnow = 0x0,
}

impl From<u32> for SgipType {
	fn from(id: u32) -> Self {
		match id {
			0x00000004 => SgipType::SgipSubmit,
			0x80000004 => SgipType::SgipSubmitResp,
			0x00000005 => SgipType::SgipDeliver,
			0x80000005 => SgipType::SgipDeliverResp,
			0x00000001 => SgipType::SgipConnect,
			0x80000001 => SgipType::SgipConnectResp,
			0x00000002 => SgipType::SgipTerminate,
			0x80000002 => SgipType::SgipTerminateResp,
			0x00000006 => SgipType::SgipQuery,
			0x80000006 => SgipType::SgipQueryResp,
			0x00000007 => SgipType::SgipCancel,
			0x80000007 => SgipType::SgipCancelResp,
			0x00000008 => SgipType::SgipActiveTest,
			0x80000008 => SgipType::SgipActiveTestResp,
			0x00000009 => SgipType::SgipFwd,
			0x80000009 => SgipType::SgipFwdResp,
			0x00000010 => SgipType::SgipMtRoute,
			0x80000010 => SgipType::SgipMtRouteResp,
			0x00000011 => SgipType::SgipMoRoute,
			0x80000011 => SgipType::SgipMoRouteResp,
			0x00000012 => SgipType::SgipGetMtRoute,
			0x80000012 => SgipType::SgipGetMtRouteResp,
			0x00000013 => SgipType::SgipMtRouteUpdate,
			0x80000013 => SgipType::SgipMtRouteUpdateResp,
			0x00000014 => SgipType::SgipMoRouteUpdate,
			0x80000014 => SgipType::SgipMoRouteUpdateResp,
			0x00000015 => SgipType::SgipPushMtRouteUpdate,
			0x80000015 => SgipType::SgipPushMtRouteUpdateResp,
			0x00000016 => SgipType::SgipPushMoRouteUpdate,
			0x80000016 => SgipType::SgipPushMoRouteUpdateResp,
			0x00000017 => SgipType::SgipGetMoRoute,
			0x80000017 => SgipType::SgipGetMoRouteResp,
			_ => SgipType::UnKnow,
		}
	}
}

///Sgip协议3.0的处理
#[derive(Debug, Default)]
pub struct Sgip {
	length_codec: LengthDelimitedCodec,
}


impl Protocol for Sgip {
	fn encode_receipt(&self,  status: SmsStatus<()>, json: &mut JsonValue) -> Option<BytesMut> {
		todo!()
	}

	fn encode_from_entity_message(&self, json: &JsonValue) -> Result<(BytesMut,Vec<u32>), Error> {
		unimplemented!()
	}


	fn encode_login_msg(&self, sp_id: &str, password: &str, version: &str) -> Result<BytesMut, Error> {
		unimplemented!()
	}

	fn encode_login_rep(&self, json: &SmsStatus<JsonValue>) -> BytesMut {
		unimplemented!()
	}

	fn decode_read_msg(&self, buf: &mut BytesMut) -> io::Result<Option<JsonValue>> {
		unimplemented!()
	}

	fn get_type_id(&self, t: MsgType) -> u32 {
		unimplemented!()
	}

	fn get_type_enum(&self, v: u32) -> MsgType {
		unimplemented!()
	}

	fn get_status_id<T>(&self, t: &SmsStatus<T>) -> u32 {
		unimplemented!()
	}

	fn get_status_enum<T>(&self, v: u32, json: T) -> SmsStatus<T> {
		unimplemented!()
	}
}

impl Clone for Sgip {
	fn clone(&self) -> Self {
		Sgip::new()
	}
}

impl Sgip {
	pub fn new() -> Self {
		Sgip {
			length_codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}
}

impl Encoder<BytesMut> for Sgip {
	type Error = io::Error;

	fn encode(&mut self, item: BytesMut, dst: &mut BytesMut) -> Result<(), Self::Error> {
		dst.extend_from_slice(&*item);

		Ok(())
	}
}

impl Decoder for Sgip {
	type Item = JsonValue;
	type Error = io::Error;

	fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		unimplemented!()
	}
}

