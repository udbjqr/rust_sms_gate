use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use json::JsonValue;
use log::{error, info, warn};
use tokio::{io, time};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::entity::{Entity, get_entity};
use crate::entity::tcp_handle::handle_rev_msg;
use crate::get_runtime;
use crate::protocol::{MsgType, Protocol, SmsStatus::{self, AuthError, MessageError, Success}};

pub struct Channel<T> {
	///是否经过认证。如果不需要认证,或者认证已经通过。此值为true.
	need_approve: bool,
	protocol: T,
	rx: Option<mpsc::Receiver<JsonValue>>,
	tx: Option<mpsc::Sender<JsonValue>>,
	rx_limit: u32,
	tx_limit: u32,
}

impl<T> Channel<T>
	where T: Protocol + Decoder<Item=JsonValue, Error=io::Error> + Encoder<BytesMut, Error=io::Error> + Clone + std::marker::Send {
	pub fn new(protocol: T, need_approve: bool) -> Channel<T> {
		Channel {
			need_approve,
			protocol,
			// entity,
			tx: None,
			rx: None,
			rx_limit: 0,
			tx_limit: 0,
		}
	}

	///开启通道连接动作。这个动作在通道已经连通以后进行
	pub async fn start_connect(&mut self, stream: TcpStream, user_name: &str, password: &str, version: u8) -> Result<(), io::Error> {
		info!("启动Channel.开始连接服务端。");

		let mut framed = Framed::new(stream, self.protocol.clone());

		if let Err(e) = self.connect(&mut framed, user_name, password, version).await {
			return Err(e);
		};

		self.start_work(&mut framed).await;

		Ok(())
	}

	///连接服务器的动作
	async fn connect(&mut self, framed: &mut Framed<TcpStream, T>, user_name: &str, password: &str, version: u8) -> Result<(), io::Error> {
		match self.protocol.e_login_msg(user_name, password, version) {
			Ok(msg) => framed.send(msg).await?,
			Err(e) => error!("生成消息出现异常。{}", e),
		}

		match framed.next().await {
			Some(Ok(resp)) => {
				//判断返回类型和返回状态。
				match (self.protocol.get_type_enum(resp["t"].as_u32().unwrap()),
				       self.protocol.get_status_enum(resp["status"].as_u32().unwrap())) {
					(MsgType::ConnectResp, SmsStatus::Success(_)) => {
						self.handle_login(resp).await;
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
			if let Err(_) = self.wait_conn(&mut framed).await {
				return;
			};
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

		let tx = self.tx.as_ref().unwrap().clone();
		let rx = self.rx.as_mut().unwrap();

		//上一次执行的时间戳
		let mut curr_tx: u32 = 0;
		let mut curr_rx: u32 = 0;

		let sleep = time::sleep(Duration::from_millis(1000));
		tokio::pin!(sleep);

		loop {
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

							let tx = tx.clone();
							get_runtime().spawn(handle_rev_msg(json, tx));
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
				send = rx.recv(),if curr_tx < self.tx_limit => {
					match send{
						Some(send) => {
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.e_message(&send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}",e);
								}else{
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
			t:"disconnect"
		};

		if let Err(e) = self.tx.as_ref().unwrap().send(dis).await {
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

		let entity = match get_entity(id).await {
			None => {
				error!("user_id找不到对应的entity.");
				return AuthError;
			}
			Some(en) => en
		};

		let (resp, rx_limit, tx_limit, rx, tx) = entity.login_attach(longin_info);
		if let Success(v) = resp {
			// 设置相关的参数
			self.rx_limit = rx_limit;
			self.tx_limit = tx_limit;
			self.rx = rx;
			self.tx = tx;

			Success(v)
		} else {
			resp
		}
	}
}



