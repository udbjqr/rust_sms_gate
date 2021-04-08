use log::{error, info};
use tokio::io;
use tokio::net::TcpListener;

use crate::entity::channel::Channel;
use crate::get_runtime;
use crate::global::load_config_file;
use crate::protocol::Protocol;

///服务器管理类。在某些端口进行开放。
#[derive(Debug, Default)]
pub struct ServersManager {}


impl ServersManager {
	///启动服务准备接受接入
	pub fn start() -> Result<(), io::Error> {
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
				info!("开始启动服务,host:{},type:{}", host, server_type);
				start_service(host, server_type).await;
			});
		}

		Ok(())
	}
}

///启动一个服务等待连接
async fn start_service(host: String, server_type: Protocol) {
	let listener = match TcpListener::bind(host.as_str()).await {
		Ok(l) => l,
		Err(e) => {
			error!("进行初始化服务端口失败。失败的host:{}.. err:{}", host, e);
			return;
		}
	};

	loop {
		let (socket, _) = match listener.accept().await {
			Ok((socket, addr)) => {
				info!("host:{}接到从{}来的连接。连接已建立。准备接收连接。", host, addr);
				(socket, addr)
			}
			Err(e) => {
				error!("接入失败。接收的host:{}.. err:{}", host, e);
				return;
			}
		};
		let server_type = server_type.clone();
		get_runtime().spawn(async move {
			let mut channel = Channel::new(server_type, true);
			channel.start_server(socket).await;
		});
	}
}
