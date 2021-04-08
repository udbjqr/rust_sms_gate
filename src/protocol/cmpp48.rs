use crate::protocol::implements::ProtocolImpl;
use tokio_util::codec::{LengthDelimitedCodec, Decoder};
use bytes::BytesMut;
use tokio::io;

///CMPP协议3.0的处理
#[derive(Debug, Default)]
pub struct Cmpp48 {
	version: u32,
	codec: LengthDelimitedCodec,
}

impl ProtocolImpl for Cmpp48 {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>>  {
		self.codec.decode(buf)
	}
}

impl Clone for Cmpp48 {
	fn clone(&self) -> Self {
		Cmpp48::new()
	}
}

impl Cmpp48 {
	pub fn new() -> Self {
		Cmpp48 {
			version: 48,
			codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}
}
