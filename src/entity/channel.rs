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
use crate::protocol::names::{STATUS, ENTITY_ID, WAIT_RECEIPT, ADDRESS, MSG_TYPE_STR, VERSION, LOGIN_NAME};
use std::time::{Instant};
use crate::entity::attach_custom::check_custom_login;
use std::net::{IpAddr, SocketAddr};
use crate::global::{message_sender, TOPIC_TO_B_FAILURE};

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
		info!("启动Channel.开始连接服务端。login_msg:{}", login_msg);

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
			log::info!("这里收到登录异常??");
			return Err(e);
		};

		*framed.codec_mut() = self.protocol.clone();
		self.start_work(&mut framed).await;

		Ok(())
	}

	///连接服务器的动作
	async fn connect(&mut self, framed: &mut Framed<TcpStream, Protocol>, entity_id: u32, mut login_msg: JsonValue, ip_addr: IpAddr) -> Result<(), io::Error> {
		match self.protocol.encode_message(&mut login_msg) {
			Ok(msg) => framed.send(msg).await?,
			Err(e) => {
				error!("生成消息出现异常。{}", e);
				return Err(io::Error::new(io::ErrorKind::InvalidData, e));
			}
		}

		match framed.next().await {
			Some(Ok(mut resp)) => {
				resp[ENTITY_ID] = entity_id.into();
				log::info!("收到登录返回信息:{}", resp);

				//判断返回类型和返回状态。
				match (resp[MSG_TYPE_STR].as_str().unwrap_or("").into(),
				       self.protocol.get_status_enum(resp[STATUS].as_u32().unwrap())) {
					(MsgType::ConnectResp, SmsStatus::Success) => {
						if let (Success, resp) = self.handle_login(resp, false, ip_addr).await {
							if let Some(version) = resp[VERSION].as_u32() {
								self.protocol = self.protocol.match_version(version);
								log::info!("登录成功 ..更换版本.现在版本:{:?}", self.protocol);
							}
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
		log::debug!("连接成功.channel准备处理数据.{}", self.id);

		let mut active_test = json::object! {
					msg_type : "ActiveTest"
		};

		let channel_to_entity_tx = if self.channel_to_entity_tx.is_some() {
			self.channel_to_entity_tx.as_ref().unwrap()
		} else {
			log::error!("没有向实体发送的通道。直接退出。{:?}", self);
			return;
		};

		if self.need_approve {
			error!("还未进行认证。退出");
			return;
		}

		let entity_to_channel_priority_rx = self.entity_to_channel_priority_rx.as_mut().unwrap();
		let entity_to_channel_common_rx = self.entity_to_channel_common_rx.as_mut().unwrap();

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
						error!("发送心跳回执出现错误, e:{}", e);
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
							log::info!("通道收到消息:{}",&json);
              if curr_rx <= self.rx_limit {
								//收到消息,生成回执...当收到的不是回执才回返回Some.
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
							self.clear().await;
							info!("当前连接已经断开。。。。");
							//连接断开的处理。
							self.tell_entity_disconnect().await;
							return;
						}
				  }
				}
				msg = entity_to_channel_priority_rx.recv(),if curr_tx < self.tx_limit => {
					idle_count = 0;
					match msg {
						Some(mut send) => {
							log::debug!("priority收到entity发来的消息.msg:{}",send);
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.encode_message(&mut send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 计数加1
									curr_tx = curr_tx + 1;
									//把要等待回复的消息再发送回实体
									send[WAIT_RECEIPT] = true.into();
									if let Err(e) = channel_to_entity_tx.send(send).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
										return;
									}
								}
							}
						}
						None => {
							warn!("实体向通道(优先)已经被关闭。直接退出。");
							self.clear().await;
							return;
						}
					}
				}
				msg = entity_to_channel_common_rx.recv(),if curr_tx < self.tx_limit => {
					idle_count = 0;
					match msg {
						Some(mut send) => {
							log::debug!("common收到entity发来的消息.msg:{}",send);
							//接收发来的消息。并处理
							if let Ok(msg) = self.protocol.encode_message(&mut send) {
								if let Err(e) = framed.send(msg).await {
									error!("发送回执出现错误, e:{}", e);
								} else {
									// 成功计数加1
									curr_tx = curr_tx + 1;
									//把要等待回复的消息再发送回实体
									send[WAIT_RECEIPT] = true.into();
									if let Err(e) = channel_to_entity_tx.send(send).await {
										log::error!("向实体发送消息出现异常, e:{}", e);
										return;
									}
								}
							}
						}
						None => {
							warn!("实体向通道(普通)已经被关闭。直接退出。");
							self.clear().await;
							return;
						}
					}
				}
				_ = time::sleep(one_secs) => {
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
			msg_type:"close",
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
								*framed.codec_mut() = self.protocol.clone();
								if let Some(msg) = self.protocol.encode_receipt(Success, &mut result) {
									framed.send(msg).await?;
								}
								self.need_approve = false;

								Ok(())
							}
							_ => {
								let name: &str = status.into();
								result[STATUS] = name.into();
								if let Some(msg) = self.protocol.encode_receipt(status, &mut result) {
									framed.send(msg).await?;
								}

								Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!("{}", status)))
							}
						}
					}
					//这里只处理登录请求。第一个收到的消息不是登录。直接退出。
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
		let manage = EntityManager::get_entity_manager();
		let entitys = manage.entitys.read().await;

		let entity = if is_server {
			if let Some((_, en)) = entitys.iter().find(|(_, en)| {
				en.get_login_name() == login_info[LOGIN_NAME].as_str().unwrap_or("")
			}) {
				en
			} else {
				log::error!("没有找到对应的login_name.退出.msg:{}", login_info);
				return (SmsStatus::AuthError, login_info);
			}
		} else {
			match login_info[ENTITY_ID].as_u32() {
				None => {
					error!("数据结构内没有entity_id的值。无法继续..info:{}", login_info);
					return (MessageError, login_info);
				}
				Some(id) => {
					match entitys.get(&id) {
						None => {
							error!("user_id找不到对应的entity.");
							return (SmsStatus::AuthError, login_info);
						}
						Some(en) => en
					}
				}
			}
		};

		if is_server {
			if let Some(result_err) = check_custom_login(&login_info, entity, &mut self.protocol, ip_addr) {
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

	async fn clear(&mut self) {
		log::debug!("通道关闭过程.{}", self.id);

		let sender = message_sender();
		if let Some(entity_to_channel_priority_rx) = self.entity_to_channel_priority_rx.as_mut() {
			while let Some(msg) = entity_to_channel_priority_rx.recv().await {
				sender.send(TOPIC_TO_B_FAILURE, "", msg.to_string().as_str()).await;
			}
		}

		if let Some(entity_to_channel_common_rx) = self.entity_to_channel_common_rx.as_mut() {
			while let Some(msg) = entity_to_channel_common_rx.recv().await {
				sender.send(TOPIC_TO_B_FAILURE, "", msg.to_string().as_str()).await;
			}
		}
	}
}
