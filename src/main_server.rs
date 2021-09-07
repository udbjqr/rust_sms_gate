#![allow(unused)]

use std::{io, net::{IpAddr, Ipv4Addr}};

use futures::{SinkExt, StreamExt};
use json::JsonValue;
use sms_gate::{entity::{CustomEntity}, get_runtime, global::{get_sequence_id, load_config_file}, protocol::{Cmpp48, MsgType, Protocol, ProtocolImpl, SmsStatus, cmpp32::Cmpp32, names::{DEST_IDS, ID, MSG_TYPE_STR, WAIT_RECEIPT}}};
use tokio::{net::{TcpListener, TcpStream}, sync::mpsc::{self, Sender}, time::{self, Duration, timeout}};
use tokio_util::codec::Framed;
use std::time::{Instant};

fn main() {
	// simple_logger::SimpleLogger::init(Default::default());
	log4rs::init_file("config/log_server.yaml", Default::default()).unwrap();


	//使用enter 加载运行时。必须需要let _guard 要不没有生命周期。
	let runtime = get_runtime();
	let _guard = runtime.enter();
	// let (manager_to_entity, mut manager_from_entity) = mpsc::channel::<JsonValue>(0xffffffff);
	let (entity_to_manager, mut _entity_from_manager) = mpsc::channel::<JsonValue>(0xffffffff);
	
	let entity = Box::new(CustomEntity::new(
		999,
		"测试服务器1".to_owned(),
		"未知".to_owned(),
		"999999".to_owned(),
		"".to_owned(),
		"999999".to_owned(),
		"123456".to_owned(),
		"0.0.0.0".to_owned(),
		500,
		500,
		3,
		json::object!{},
		entity_to_manager.clone()
	));

	get_runtime().spawn(start_service("127.0.0.1:5000".to_owned(),Protocol::CMPP48(Cmpp48::new())));

	std::thread::park();
}

async fn start_server(stream: TcpStream,protocol: Protocol){
	log::info!("启动Channel.准备接受连接。");

	let ip_addr = match stream.peer_addr(){
		Ok(addr) => addr.ip(),
		Err(e) => {
			log::error!("得到IP地址出现错误。e:{}", e);
			IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
		}
	};

	let p2 = protocol.clone();
	let mut framed = Framed::new(stream, protocol);

	start_work(&mut framed,p2).await;
}


///每一个通道的发送和接收处理。
/// 这里应该已经处理完接收和发送的消息。
/// 送到这里的都是单个短信的消息
async fn start_work(framed: &mut Framed<TcpStream, Protocol>,protocol: Protocol) {
	let (entity_to_channel_tx, mut entity_to_channel_rx) = mpsc::channel::<JsonValue>(0xffffffff);

	let tx_limit = 1000;
	let rx_limit = 1000u32;
	let id = get_sequence_id(1);
	log::debug!("连接成功.channel准备处理数据.id:{}", id);

	let mut active_test = json::object! {
		msg_type : "ActiveTest"
	};

	let mut curr_tx: u32 = 0;
	let mut curr_rx: u32 = 0;
	let mut idle_count: u16 = 0;
	
	let mut wait_active_resp = false;

	let mut timestamp = Instant::now();
	let one_secs = Duration::from_secs(1);

	loop {
		//一个时间窗口过去.重新计算
		if timestamp.elapsed() > one_secs {
			curr_tx = 0;
			curr_rx = 0;
			timestamp = Instant::now();
		}

		//根据当前是否已经发满。发送当前是否可用数据。
		tokio::select! {
			biased;
			msg = entity_to_channel_rx.recv(),if curr_tx < tx_limit => {
				idle_count = 0;
				match msg {
					Some(mut send) => {
						log::debug!("priority收到entity发来的消息.msg:{}",send);
						//接收发来的消息。并处理
						if let Ok(msg) = protocol.encode_message(&mut send) {
							log::info!("{}向对端发送消息{}", id, &send);
							if let Err(e) = framed.send(msg).await {
								log::error!("发送回执出现错误, e:{}", e);
							}
						}
					}
					None => {
						log::warn!("实体向通道(优先)已经被关闭。直接退出。");
					}
				}
			}
			msg = framed.next() => {
				match msg {
					Some(Ok(mut json)) => {
						log::info!("{}通道收到消息:{}",id,&json);
						wait_active_resp = false;

						let ty = json[MSG_TYPE_STR].as_str().unwrap_or("").into();

						match ty {
							MsgType::Submit => {
								//当消息需要需要返回的才记录接收数量
								curr_rx = curr_rx + 1;
							}
							MsgType::Terminate => {
								//收到终止消息。将ID带上
								json[ID] = id.into();
							}
							_ => {
								//这里目前不用做处理
							}
						}

						//只有发送短信才进行判断
						if ty != MsgType::Submit || curr_rx <= rx_limit {
							//生成回执...当收到的是回执才回返回Some.
							if let Some(resp) = protocol.encode_receipt(SmsStatus::Success, &mut json) {
								if let Err(e) = framed.send(resp).await {
									log::error!("发送回执出现错误, e:{}", e);
								}
							}

							if ty == MsgType::Submit {
								//这里准备处理做返回
								get_runtime().spawn(return_msg(json,protocol.clone(),entity_to_channel_tx.clone()));
							}
						} else {
							// 超出,返回流量超出
							if let Some(resp) = protocol.encode_receipt(SmsStatus::TrafficRestrictions, &mut json) {
								log::debug!("当前超出流量.json:{}.已发:{}.可发:{}", json, curr_rx, rx_limit);

								if let Err(e) = framed.send(resp).await{
									log::error!("发送回执出现错误, e:{}",e);
								}
							}
						}
					}
					Some(Err(e)) => {
						log::error!("解码出现错误,跳过当前消息。{}", e);
					}
					None => {
						log::info!("当前连接已经断开。。。。id:{}",id);

						//连接断开的处理。
						return;
					}
				}
			}
			//用来判断限制发送窗口期已过
			_ = time::sleep(Instant::now() - timestamp),if curr_tx >= tx_limit => {}
			_ = time::sleep(one_secs) => {
				//这里就是用来当全部都没有动作的时间打开再次进行循环.
				//空闲记数
				idle_count = idle_count + 1;
			}
		}
	}

	log::info!("当前通道结束退出.{}",id);
}




///启动服务准备接受接入
fn start() -> Result<(), io::Error> {
	let mut config = load_config_file("config/smsServer.json");
	//取数组
	let array = config["config"].take();

	for item in array.members() {
		let host = match item["host"].as_str() {
			Some(h) => h.to_string(),
			None => {
				return Err(io::Error::new(io::ErrorKind::NotFound, "host为空。不能使用"));
			}
		};

		let server_type: Protocol = match item["server_type"].as_str() {
			Some(h) => h.into(),
			None => {
				return Err(io::Error::new(io::ErrorKind::NotFound, "server_type为空。不能使用"));
			}
		};


		get_runtime().spawn(async move {
			log::info!("开始启动服务,host:{},type:{}", host, server_type);
			start_service(host, server_type).await;
		});
	}

	Ok(())
}

///启动一个服务等待连接
async fn start_service(host: String, server_type: Protocol) {
	log::info!("等待接入,host:{},type:{}", host, server_type);
	let listener = match TcpListener::bind(host.as_str()).await {
		Ok(l) => l,
		Err(e) => {
			log::error!("进行初始化服务端口失败。失败的host:{}.. err:{}", host, e);
			return;
		}
	};

	loop {
		let (socket, _) = match listener.accept().await {
			Ok((socket, addr)) => {
				log::info!("host:{}接到从{}来的连接。连接已建立。准备接收连接。", host, addr);
				(socket, addr)
			}
			Err(e) => {
				log::error!("接入失败。接收的host:{}.. err:{}", host, e);
				return;
			}
		};
		let server_type = server_type.clone();

		get_runtime().spawn(async move {
			start_server(socket,server_type).await;
		});
	}
}

async fn return_msg(json: JsonValue,protocol: Protocol,sender: Sender<JsonValue>){
	// let time = rand::random::<u64>() % 2000 + 50u64;
	let time = 5000;
	let duration = Duration::from_millis(time);
	tokio::time::sleep(duration).await;
	let d = json[DEST_IDS][0].as_str().unwrap_or("");
	let msg = json::object!{
		msg_fmt: 0,
		src_id: json["src_id"].as_str().unwrap_or(""),
		submit_time: "2108052003",
		serviceId: "",
		msg_type: "Report",
		state: "DELIVRD",
		dest_id: d,
		msg_id: json["msg_id"].as_str().unwrap_or(""),
		done_time: "2108052003"
	};

	sender.send(msg).await;
}