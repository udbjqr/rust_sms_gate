#![allow(unused)]


use chrono::{DateTime, Local, Datelike, Timelike};
use crate::cmpp::{Cmpp, CmppType};
use bytes::{BytesMut, BufMut};
use json::JsonValue;
use tokio_util::codec::{Encoder, LengthDelimitedCodec, Decoder};

#[test]
fn test_md5() {
	let d = md5::compute("1231");
	assert_eq!("6c14da109e294d1e8155be8aa4b1ce8e", format!("{:x}", d));
	for v in d.iter() {
		print!("{:x} ", v);
	}
}



#[test]
fn test_time() {
	let local: DateTime<Local> = Local::now();

	let mut time: u32 = local.month();
	time = time * 100 + local.day();
	time = time * 100 + local.hour();
	time = time * 100 + local.minute();
	time = time * 100 + local.second();

	println!("{}", format!("ddd:{:010}", time))
}


#[test]
fn test_type() {
   let time = 3322;
	let time_str = format!("{:010}", time);
	println!("{}",time_str)
}





#[test]
fn test_length_delimited() {
	let mut buf = BytesMut::with_capacity(20);
	buf.put_u32(20);
	buf.put_u32(90);
	buf.put_u32(90);
	buf.put_u32(0x00);
	buf.put_u32(0x00);
	buf.put_u32(0xffff);
	println!("打印:{:x?}",buf);

	let mut codec = LengthDelimitedCodec::builder()
		.length_field_offset(0) // default value
		.length_field_length(4)
		.length_adjustment(-4)   // default value
		.new_codec();


	let v  = codec.decode(&mut buf).unwrap().unwrap();

	println!("打印:{:x?}",v);
}


#[test]
fn test_left_time() {
	let mut buf = 20;
	{
		buf =3;
	}

	println!("{}",buf);

}

