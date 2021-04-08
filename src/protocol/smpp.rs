use tokio_util::codec::{LengthDelimitedCodec, Decoder};
use crate::protocol::implements::ProtocolImpl;
use bytes::BytesMut;
use tokio::io;

///Sgip协议的处理
#[derive(Debug, Default)]
pub struct Smpp {
	version:u32,
	length_codec: LengthDelimitedCodec,
}


impl ProtocolImpl for Smpp {
	fn get_framed(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>>  {
		self.length_codec.decode(buf)
	}
}

impl Clone for Smpp {
	fn clone(&self) -> Self {
		Smpp::new()
	}
}

impl Smpp {
	pub fn new() -> Self {
		Smpp {
			version: 0,
			length_codec: LengthDelimitedCodec::builder()
				.length_field_offset(0)
				.length_field_length(4)
				.length_adjustment(-4)
				.new_codec(),
		}
	}
}
