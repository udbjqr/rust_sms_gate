use chrono::{DateTime, Local, Datelike, Timelike};
mod entity;

#[test]
fn test_time() {
	let local: DateTime<Local> = Local::now();
	let mut time: u32 = local.month();
	time = time * 100 + local.day();
	time = time * 100 + local.hour();
	time = time * 100 + local.minute();
	time = time * 100 + local.second();

	let time_str = format!("{:010}", time);

	dbg!(time,time_str);
}

