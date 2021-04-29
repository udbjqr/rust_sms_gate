#![allow(unused)]


use sms_gate::entity::channel::Channel;
use tokio::net::{TcpSocket, TcpStream};
use sms_gate::get_runtime;
use tokio_util::codec::Framed;
use sms_gate::protocol::Protocol::{CMPP32, CMPP48, SMGP};
use sms_gate::protocol::cmpp32::Cmpp32;
use sms_gate::protocol::{Cmpp48, ProtocolImpl, MsgType, SmsStatus, Protocol};
use futures::{SinkExt, StreamExt};
use tokio::io;
use sms_gate::protocol::names::{ENTITY_ID, MSG_TYPE_STR, STATUS, VERSION};
use std::time::{Instant, Duration};
use json::JsonValue;
use tokio::sync::mpsc;
use sms_gate::protocol::smgp::Smgp;

fn main() {
	simple_logger::SimpleLogger::init(Default::default());
	get_runtime().spawn(async move {
		// let addr = "118.31.45.242:7890".parse().unwrap();
		let addr = "219.146.23.81:5020".parse().unwrap();
		let socket = TcpSocket::new_v4().unwrap();
		let stream = socket.connect(addr).await.unwrap();

		let mut protocol = SMGP(Smgp::new());
		let mut framed = Framed::new(stream, protocol.clone());

		let mut login_msg = json::object! {
				loginName: "101016",
				password: "S6#j7Fgc!CXe",
				// gatewayIp: "118.31.45.242:7890",
				// loginName: "101016",
				// password: "S6#j7Fgc!CXe",
				protocolVersion: 0x30u32,
				msg_type: "Connect"
			};

		match protocol.encode_message(&mut login_msg) {
			Ok(msg) => framed.send(msg).await.unwrap(),
			Err(e) => {
				println!("生成消息出现异常。{}", e);
				return Err(io::Error::new(io::ErrorKind::InvalidData, e));
			}
		}

		let result = match framed.next().await {
			Some(Ok(mut resp)) => {
				resp[ENTITY_ID] = 99.into();
				println!("收到登录返回信息:{}", resp);

				//判断返回类型和返回状态。
				match (resp[MSG_TYPE_STR].as_str().unwrap_or("").into(),
				       protocol.get_status_enum(resp[STATUS].as_u32().unwrap())) {
					(MsgType::ConnectResp, SmsStatus::Success) => {
						if let Some(version) = resp[VERSION].as_u32() {
							protocol = protocol.match_version(version);
							println!("登录成功 ..更换版本.现在版本:{:?}", protocol);
						}

						Some(())
					}
					_ => {
						println!("登录被拒绝.msg:{}", resp);
						None
					}
				}
			}
			Some(Err(e)) => {
				println!("这里是解码错误?? err = {:?}", e);
				None
			}
			None => {
				println!("连接已经断开");
				None
			}
		};

		if result.is_some() {
			start_work(&mut framed, protocol).await;
		}

		Ok(())
	});

	std::thread::park();
}

struct Peer {
	rx: mpsc::UnboundedReceiver<JsonValue>,
}


async fn start_work(framed: &mut Framed<TcpStream, Protocol>, protocol: Protocol) {
	println!("连接成功.channel准备处理数据.");

	let mut active_test = json::object! {
					msg_type : "ActiveTest"
		};



	let one_secs = Duration::from_secs(1);
	let (tx,rx) = mpsc::unbounded_channel();

	get_runtime().spawn(async move{
		let mut count = 0u32;
		let json = json::object! {
			msg_content: "【签名】测试",
			serviceId: "99",
			spId: "101016",
			src_id: "106830741234567",
			msg_type:"Submit",
			dest_ids:[
				"17333173834"
			],
			msg_ids:["042911301994803057760"]
		};

		for i in 0..1 {
			if let Err(e) =	tx.send(json.clone()){
				println!("发送消息错误:{}",e);
			}
		};
	});

	let mut peer = Peer{rx};

	loop {
		//根据当前是否已经发满。发送当前是否可用数据。
		tokio::select! {
			Some(msg) = peer.rx.recv() => {
				let mut msg = msg;
				let send_msg = protocol.encode_message(&mut msg).unwrap();
				if let Err(e) = framed.send(send_msg).await {
						println!("发送出现错误:{}",e);
				}
			}
			msg = framed.next() => {
			  match msg {
			    Some(Ok(mut json)) => {
						println!("通道收到消息:{}",&json);
						//收到消息,生成回执...当收到的不是回执才回返回Some.
						if let Some(resp) = protocol.encode_receipt(SmsStatus::Success, &mut json) {
							if let Err(e) = framed.send(resp).await {
								println!("发送回执出现错误, e:{}", e);
							}
						}
					}
					Some(Err(e)) => {
						println!("解码出现错误,跳过当前消息。{}", e);
					}
					None => {
						println!("当前连接已经断开。。。。");
						return;
					}
			  }
			}
			_ = tokio::time::sleep(one_secs) => {
				//这里就是用来当全部都没有动作的时间打开再次进行循环.
				//空闲记数
			}
		}
	}
}
