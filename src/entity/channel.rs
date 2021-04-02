use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use json::JsonValue;
use log::{error, info, warn};
use tokio::{io, time};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::entity::EntityManager;
use crate::entity::tcp_handle::handle_rev_msg;
use crate::get_runtime;
use crate::protocol::{MsgType, Protocol, SmsStatus::{self, MessageError, Success}};

pub struct Channel<T> {
	id: usize,
	///是否经过认证。如果不需要认证,或者认证已经通过。此值为true.
	need_approve: bool,
	protocol: T,
	entity_to_channel_priority_rx: Option<mpsc::Receiver<JsonValue>>,
	entity_to_channel_common_rx: Option<mpsc::Receiver<JsonValue>>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
	rx_limit: u32,
	tx_limit: u32,
}

impl<T> Channel<T>
	where T: Protocol + Decoder<Item=JsonValue, Error=io::Error> + Encoder<BytesMut, Error=io::Error> + Clone + std::marker::Send {
	pub fn new(protocol: T, need_approve: bool) -> Channel<T> {
		Channel {
			id: 0,
			need_approve,
			protocol,
			// entity,
			entity_to_channel_priority_rx: None,
			entity_to_channel_common_rx: None,
			channel_to_entity_tx: None,
			rx_limit: 0,
			tx_limit: 0,
		}
	}

	///开启通道连接动作。这个动作在通道已经连通以后进行
	pub async fn start_connect(&mut self, id: u32, address: &str, user_name: &str, password: &str, version: &str) -> Result<(), io::Error> {
		info!("启动Channel.开始连接服务端。");

		let addr = address.parse().unwrap();
		let socket = TcpSocket::new_v4()?;
		let stream = socket.connect(addr).await?;
		let mut framed = Framed::new(stream, self.protocol.clone());

		if let Err(e) = self.connect(&mut framed, id, user_name, password, version).await {
			return Err(e);
		};

		self.start_work(&mut framed).await;

		Ok(())
	}

	///连接服务器的动作
	async fn connect(&mut self, framed: &mut Framed<TcpStream, T>, id: u32, user_name: &str, password: &str, version: &str) -> Result<(), io::Error> {
		match self.protocol.e_login_msg(user_name, password, version) {
			Ok(msg) => framed.send(msg).await?,
			Err(e) => error!("生成消息出现异常。{}", e),
		}

		match framed.next().await {
			Some(Ok(mut resp)) => {
				//判断返回类型和返回状态。
				match (self.protocol.get_type_enum(resp["t"].as_u32().unwrap()),
				       self.protocol.get_status_enum(resp["status"].as_u32().unwrap())) {
					(MsgType::ConnectResp, SmsStatus::Success(_)) => {
						resp["user_id"] = id.into();
						if let Success(_) = self.handle_login(resp).await {
							info!("登录成功。");
						} else {
							error!("登录后初始化异常。");
						}

						Ok(())
					}
					_ => Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("登录出错。{}", resp)))
				}
			}
			Some(Err(e)) => {
				error!("这里是解码错误?? err = {:?}", e);
				Err(io::Error::new(io::ErrorKind::PermissionDenied, e))
			}
			None => {
				Err(io::Error::new(io::ErrorKind::Other, "连接已经断开!!"))
			}
		}
	}

	///开启服务。等待接收客户端信息。
	pub async fn start_server(&mut self, stream: TcpStream) {
		info!("启动Channel.准备接受连接。");

		let mut framed = Framed::new(stream, self.protocol.clone());
		if self.need_approve {
			if let Err(e) = self.wait_conn(&mut framed).await {
				log::error!("记录一下登录的错误。e:{}", e);
				return;
			}
		}

		self.start_work(&mut framed).await;
	}

	///每一个通道的发送和接收处理。
	/// 这里应该已经处理完接收和发送的消息。
	/// 送到这里的都是单个短信的消息
	async fn start_work(&mut self, framed: &mut Framed<TcpStream, T>) {
		if !self.need_approve {
			error!("还未进行认证。退出");
			return;
		}

		let entity_to_channel_priority_rx = self.entity_to_channel_priority_rx.as_mut().unwrap();
		let entity_to_channel_common_rx = self.entity_to_channel_common_rx.as_mut().unwrap();

		//上一次执行的时间戳
		let mut curr_tx: u32 = 0;
		let mut curr_rx: u32 = 0;

		let sleep = time::sleep(Duration::from_millis(1000));
		tokio::pin!(sleep);

		loop {
			//根据当前是否已经发满。发送当前是否可用数据。
			tokio::select! {
			  msg = framed.next(), if curr_rx < self.rx_limit => {
			    curr_rx = curr_rx + 1;
				  match msg {
				    Some(Ok(json)) => {
							// 发送回执.只有需要的才发送
							if let Some(resp) = self.protocol.e_receipt(&json) {
								if let Err(e) = framed.send(resp).await{
									error!("发送回执出现错误, e:{}",e);
								}
							}
							get_runtime().spawn(handle_rev_msg(json));
						}
						Some(Err(e)) => {
							error!("解码出现错误,跳过当前消息。{}", e)
						}
						None => {
							info!("当前连接已经断开。。。。");
							//连接断开的处理。
							self.tell_entity_disconnect().await;
							return;
						}
				  }
				}
				send = entity_to_channel_priority_rx.recv(),if curr_tx < self.tx_limit => {
					match send {
						Some(send) => {
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.e_message(&send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 计数加1
									curr_tx = curr_tx + 1;
								}
							}
						}
						None => {
							warn!("通道已经被关闭。直接退出。");
							return;
						}
					}
				}
				send = entity_to_channel_common_rx.recv(),if curr_tx < self.tx_limit => {
					match send {
						Some(send) => {
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.e_message(&send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 计数加1
									curr_tx = curr_tx + 1;
								}
							}
						}
						None => {
							warn!("通道已经被关闭。直接退出。");
							return;
						}
					}
				}
				//这里是当上面两个都未收启用时处理,也有可能随时执行到这里
				//当时间窗口过去,重新计数
				_ = &mut sleep => {
					curr_tx = 0;
					curr_rx = 0;
				}
			}
		}
	}


	async fn tell_entity_disconnect(&mut self) {
		//发送连接断开消息。
		let dis = json::object! {
			msg_type:"close",
			id:self.id,
		};

		if let Err(e) = self.channel_to_entity_tx.as_ref().unwrap().send(dis).await {
			error!("发送消息出现异常。e:{}", e)
		}
	}

	///等待连接。这里应该是做为服务端会有的操作。
	async fn wait_conn(&mut self, framed: &mut Framed<TcpStream, T>) -> Result<(), io::Error> {
		//3秒超时。
		match timeout(Duration::from_secs(3), framed.next()).await {
			Ok(Some(Ok(request))) => {
				match self.protocol.get_type_enum(request["t"].as_u32().unwrap()) {
					MsgType::Connect => {
						let resp = self.handle_login(request).await;
						match resp {
							Success(result) => {
								info!("登录成功。{}", result);
								//TODO 根据版本号。调整处理类型 self.protocol = ???
								let msg = self.protocol.e_login_rep_msg(&Success(result));
								framed.send(msg).await?;

								Ok(())
							}
							//这里只处理登录请求。第一个收到的消息不是登录。直接退出。
							_ => {
								Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("{}", resp)))
							}
						}
					}
					_ => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "当前需要登录请求。"))
				}
			}
			Ok(Some(Err(e))) => {
				error!("这里是解码错误?? err = {:?}", e);
				Err(io::Error::new(io::ErrorKind::PermissionDenied, e))
			}
			Ok(None) => {
				Err(io::Error::new(io::ErrorKind::Other, "连接已经断开!!"))
			}
			Err(e) => {
				warn!("连接超时。退出。");
				Err(io::Error::new(io::ErrorKind::TimedOut, e))
			}
		}
	}

	async fn handle_login(&mut self, longin_info: JsonValue) -> SmsStatus<JsonValue> {
		//未知异常返回给客户的结构。
		let id = match longin_info["user_id"].as_u32() {
			None => {
				error!("数据结构内没有user_id的值。无法继续");
				return MessageError;
			}
			Some(id) => id
		};

		let manage = EntityManager::get_entity_manager();
		let mut entitys = manage.entitys.write().await;
		let entity = match entitys.get_mut(&id) {
			None => {
				error!("user_id找不到对应的entity.");
				return SmsStatus::AuthError;
			}
			Some(en) => en
		};

		let (id, resp, rx_limit, tx_limit, entity_to_channel_priority_rx, entity_to_channel_common_rx, channel_to_entity_tx) = entity.login_attach(longin_info).await;
		if let Success(v) = resp {
			// 设置相关的参数
			self.id = id;
			self.rx_limit = rx_limit;
			self.tx_limit = tx_limit;
			self.entity_to_channel_priority_rx = entity_to_channel_priority_rx;
			self.entity_to_channel_common_rx = entity_to_channel_common_rx;
			self.channel_to_entity_tx = channel_to_entity_tx;

			Success(v)
		} else {
			resp
		}
	}
}
