use futures::{SinkExt, StreamExt};
use json::JsonValue;
use log::{error, info, warn};
use tokio::{io, time};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tokio_util::codec::Framed;

use crate::entity::EntityManager;
use crate::protocol::{MsgType, SmsStatus::{self, MessageError, Success}, Protocol};
use crate::protocol::names::{STATUS, USER_ID, WAIT_RECEIPT, ADDRESS, MSG_TYPE_STR};
use std::time::{Instant};
use crate::entity::attach_custom::check_custom_login;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug)]
pub struct Channel {
	id: usize,
	///是否经过认证。如果不需要认证,或者认证已经通过。此值为true.
	need_approve: bool,
	protocol: Protocol,
	entity_to_channel_priority_rx: Option<mpsc::Receiver<JsonValue>>,
	entity_to_channel_common_rx: Option<mpsc::Receiver<JsonValue>>,
	channel_to_entity_tx: Option<mpsc::Sender<JsonValue>>,
	rx_limit: u32,
	tx_limit: u32,
}

impl Channel {
	pub fn new(protocol: Protocol, need_approve: bool) -> Channel {
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
	pub async fn start_connect(&mut self, id: u32, login_msg: JsonValue) -> Result<(), io::Error> {
		info!("启动Channel.开始连接服务端。");

		let addr = match login_msg[ADDRESS].as_str() {
			Some(v) => {
				if let Ok(add) = v.parse::<SocketAddr>() {
					add
				} else {
					return Err(io::Error::new(io::ErrorKind::InvalidData, format!("错误的IPaddr值:{}", v)));
				}
			}
			None => {
				log::error!("没有address.退出..json:{}", login_msg);
				return Err(io::Error::new(io::ErrorKind::NotFound, "没有address"));
			}
		};

		let ip_addr: IpAddr = addr.ip();
		let socket = TcpSocket::new_v4()?;
		let stream = socket.connect(addr).await?;

		let mut framed = Framed::new(stream, self.protocol.clone());

		if let Err(e) = self.connect(&mut framed, id, login_msg, ip_addr).await {
			return Err(e);
		};

		self.start_work(&mut framed).await;

		Ok(())
	}

	///连接服务器的动作
	async fn connect(&mut self, framed: &mut Framed<TcpStream, Protocol>, id: u32, mut login_msg: JsonValue, ip_addr: IpAddr) -> Result<(), io::Error> {
		match self.protocol.encode_message(&mut login_msg) {
			Ok(msg) => framed.send(msg).await?,
			Err(e) => error!("生成消息出现异常。{}", e),
		}

		match framed.next().await {
			Some(Ok(mut resp)) => {
				resp[USER_ID] = id.into();
				log::info!("收到登录返回信息:{}", resp);

				//判断返回类型和返回状态。
				match (resp[MSG_TYPE_STR].as_str().unwrap_or("").into(),
				       self.protocol.get_status_enum(resp[STATUS].as_u32().unwrap())) {
					(MsgType::ConnectResp, SmsStatus::Success) => {
						if let (Success, _) = self.handle_login(resp, false, ip_addr).await {
							info!("登录成功。");
						} else {
							error!("登录后初始化异常。");
						}

						Ok(())
					}
					_ => {
						log::error!("登录被拒绝.msg:{}", resp);
						Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("登录被拒绝。")))
					}
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

		let ip_addr = stream.peer_addr().unwrap().ip();
		let mut framed = Framed::new(stream, self.protocol.clone());
		if self.need_approve {
			if let Err(e) = self.wait_conn(&mut framed, ip_addr).await {
				log::error!("记录一下登录的错误。e:{}", e);
				return;
			}
		}

		self.start_work(&mut framed).await;
	}

	///每一个通道的发送和接收处理。
	/// 这里应该已经处理完接收和发送的消息。
	/// 送到这里的都是单个短信的消息
	async fn start_work(&mut self, framed: &mut Framed<TcpStream, Protocol>) {
		let mut active_test = json::object! {
					msg_type : "ActiveTest"
		};

		let channel_to_entity_tx = if self.channel_to_entity_tx.is_some() {
			self.channel_to_entity_tx.as_ref().unwrap()
		} else {
			log::error!("没有向实体发送的通道。直接退出。{:?}", self);
			return;
		};

		if !self.need_approve {
			error!("还未进行认证。退出");
			return;
		}

		let entity_to_channel_priority_rx = self.entity_to_channel_priority_rx.as_mut().unwrap();
		let entity_to_channel_common_rx = self.entity_to_channel_common_rx.as_mut().unwrap();

		let sleep = time::sleep(Duration::from_millis(1000));
		tokio::pin!(sleep);

		//上一次执行的时间戳
		let mut curr_tx: u32 = 0;
		let mut curr_rx: u32 = 0;
		let mut idle_count: u16 = 0;

		let mut timestamp = Instant::now();
		let one_secs = Duration::from_secs(1);

		loop {
			//一个时间窗口过去,清除数据
			if timestamp.elapsed() > one_secs {
				curr_tx = 0;
				curr_rx = 0;
				timestamp = Instant::now();
			}

			//当空闲超过时间后发送心跳
			if idle_count > 30 {
				if let Ok(send_msg) = self.protocol.encode_message(&mut active_test) {
					if let Err(e) = framed.send(send_msg).await {
						error!("发送回执出现错误, e:{}", e);
					}
				} else {
					log::info!("没有得到编码完成的数据.不发送心跳.")
				}

				idle_count = 0;
			}

			//根据当前是否已经发满。发送当前是否可用数据。
			tokio::select! {
			  msg = framed.next() => {
					idle_count = 0;
				  match msg {
				    Some(Ok(mut json)) => {
              if curr_rx <= self.rx_limit {
								//当收到的不是回执才回返回Some.
								if let Some(resp) = self.protocol.encode_receipt(SmsStatus::Success,&mut json) {
									if let Err(e) = channel_to_entity_tx.send(json).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
									}

									if let Err(e) = framed.send(resp).await{
										error!("发送回执出现错误, e:{}", e);
									}
								} else {
									//把回执发回给实体处理回执
									if let Err(e) = channel_to_entity_tx.send(json).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
									}
								}
							} else {
								// 超出,返回流量超出
								if let Some(resp) = self.protocol.encode_receipt(SmsStatus::TrafficRestrictions,&mut json) {
									//当消息需要需要返回的才记录接收数量.即: 当消息为返回消息时不进行记录.
							    curr_rx = curr_rx + 1;
									if let Err(e) = framed.send(resp).await{
										error!("发送回执出现错误, e:{}",e);
									}
								}
							}
						}
						Some(Err(e)) => {
							error!("解码出现错误,跳过当前消息。{}", e);
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
					idle_count = 0;
					match send {
						Some(mut send) => {
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.encode_message(&mut send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 计数加1
									curr_tx = curr_tx + 1;
									//把收到的消息发送给实体
									send[WAIT_RECEIPT] = true.into();
									if let Err(e) = channel_to_entity_tx.send(send).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
										return;
									}
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
					idle_count = 0;
					match send {
						Some(mut send) => {
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.encode_message(&mut send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 成功计数加1
									curr_tx = curr_tx + 1;
									//把收到的消息发送给实体
									send[WAIT_RECEIPT] = true.into();
									if let Err(e) = channel_to_entity_tx.send(send).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
										return;
									}
								}
							}
						}
						None => {
							warn!("通道已经被关闭。直接退出。");
							return;
						}
					}
				}
				_ = &mut sleep => {
					//这里就是用来当全部都没有动作的时间打开再次进行循环.
					//空闲记数
					idle_count = idle_count + 1;
				}
			}
		}
	}

	async fn tell_entity_disconnect(&mut self) {
		//发送连接断开消息。
		let dis = json::object! {
			msg_type_str:"close",
			id:self.id,
		};

		if let Err(e) = self.channel_to_entity_tx.as_ref().unwrap().send(dis).await {
			error!("发送消息出现异常。e:{}", e)
		}
	}

	///等待连接。这里应该是做为服务端会有的操作。
	async fn wait_conn(&mut self, framed: &mut Framed<TcpStream, Protocol>, ip_addr: IpAddr) -> Result<(), io::Error> {
		//3秒超时。
		match timeout(Duration::from_secs(3), framed.next()).await {
			Ok(Some(Ok(request))) => {
				match request[MSG_TYPE_STR].as_str().unwrap_or("").into() {
					MsgType::Connect => {
						let (status, mut result) = self.handle_login(request, true, ip_addr).await;
						match status {
							Success => {
								info!("登录成功。{}", result);
								if let Ok(msg) = self.protocol.encode_message(&mut result) {
									framed.send(msg).await?;
								}

								Ok(())
							}
							//这里只处理登录请求。第一个收到的消息不是登录。直接退出。
							_ => {
								let name: &str = status.into();
								result[STATUS] = name.into();
								if let Ok(msg) = self.protocol.encode_message(&mut result) {
									framed.send(msg).await?;
								}

								Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("{}", status)))
							}
						}
					}
					_ => {
						log::error!("当前只处理登录消息.msg:{}", request);
						Err(io::Error::new(io::ErrorKind::ConnectionAborted, "当前需要登录请求。"))
					}
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

	///is_server 指明当前操作是否是做为服务端。是服务端将增加检验操作。
	async fn handle_login(&mut self, login_info: JsonValue, is_server: bool, ip_addr: IpAddr) -> (SmsStatus, JsonValue) {
		let id = match login_info[USER_ID].as_u32() {
			None => {
				error!("数据结构内没有user_id的值。无法继续");
				return (MessageError, login_info);
			}
			Some(id) => id
		};

		let manage = EntityManager::get_entity_manager();
		let mut entitys = manage.entitys.write().await;
		let entity = match entitys.get_mut(&id) {
			None => {
				error!("user_id找不到对应的entity.");
				return (SmsStatus::AuthError, login_info);
			}
			Some(en) => en
		};

		if is_server {
			if let Some(result_err) = check_custom_login(&login_info, entity, &self.protocol, ip_addr) {
				return result_err;
			}
		}

		let (id, status, rx_limit, tx_limit, entity_to_channel_priority_rx, entity_to_channel_common_rx, channel_to_entity_tx) = entity.login_attach().await;
		if let Success = status {
			// 设置相关的参数
			self.id = id;
			self.rx_limit = rx_limit;
			self.tx_limit = tx_limit;
			self.entity_to_channel_priority_rx = entity_to_channel_priority_rx;
			self.entity_to_channel_common_rx = entity_to_channel_common_rx;
			self.channel_to_entity_tx = channel_to_entity_tx;

			(Success, login_info)
		} else {
			(status, login_info)
		}
	}
}
