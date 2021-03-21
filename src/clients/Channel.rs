use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use log::{error, info, warn};
use tokio::io;
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, sleep};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::protocol::Protocol;

#[derive(Debug)]
pub struct Channel<T> {
	addr: String,
	user_name: String,
	password: String,
	conn_statue: Arc<AtomicBool>,
	seq: Arc<AtomicU32>,
	///通道消息互动,当前连接对应此消息
	channel_msg_rx: oneshot::Receiver<String>,
	///得到通道过来的消息。向外发送的发送器
	read_msg_tx: mpsc::Sender<String>,
	///得到消息队列来的消息。向通道发送的接收器
	write_msg_rx: mpsc::Receiver<String>,
	_inner: T,
}


impl<T> Channel<T>
	where T: Protocol + Decoder<Item=BytesMut, Error=io::Error> + Encoder<BytesMut, Error=io::Error> + Clone + std::marker::Send
{
	fn get_sequence(&self) -> u32 {
		self.seq.fetch_add(1, Ordering::Relaxed)
	}

	pub fn new(addr: String, user_name: String, password: String, conn_statue: Arc<AtomicBool>, seq: Arc<AtomicU32>, channel_msg_rx: oneshot::Receiver<String>, read_msg_tx: mpsc::Sender<String>, write_msg_rx: mpsc::Receiver<String>, protocol: T) -> Channel<T> {
		Channel {
			addr,
			user_name,
			password,
			conn_statue,
			seq,
			channel_msg_rx,
			read_msg_tx,
			write_msg_rx,
			_inner: protocol,
		}
	}

	pub async fn start_work(&mut self) {
		let mut transport: Option<Framed<TcpStream, T>> = None;
		loop {
			match self.connect().await {
				Ok(t) => {
					transport = Some(t);
					self.conn_statue.fetch_or(true, Ordering::Relaxed);

					break;
				}
				Err(_) => {
					warn!("没有连接成功,等待重试。");
					sleep(Duration::from_secs(3)).await;
				}
			}
		}


		info!("登录成功,准备接收数据..发送消息的发送器。{:?},关闭的触发器:{:?}", self.read_msg_tx, close_trigger);

		// self.start_read_data(transport);
	}

	fn start_read_data(&self, mut transport: Framed<TcpStream, T>) {
		// while let Some(request) = transport.next().await {
		// 	match request {
		// 		Ok(request) => {
		// 			get_runtime().spawn(async move {});
		//
		//
		// 			info!("打印返回的数据..{:?}", request);
		// 			// Ok(transport)
		// 		}
		// 		Err(e) => {
		// 			error!("这里是错误?? err = {:?}", e);
		// 			// Err(e.into())
		// 		}
		// 	};
		// }
	}

	async fn connect(&self) -> Result<Framed<TcpStream, T>, io::Error> {
		let socket = TcpSocket::new_v4()?;
		socket.set_reuseaddr(true)?;

		let addr = match self.addr.parse() {
			Ok(addr) => addr,
			Err(e) => return Err(io::Error::new(io::ErrorKind::BrokenPipe, e)),
		};

		let stream = socket.connect(addr).await?;
		let mut transport = Framed::new(stream, self._inner.clone());

		match self._inner.login_msg(self.user_name.as_str(), self.password.as_str(), 0x30, self.get_sequence()) {
			Ok(msg) => transport.send(msg).await?,
			Err(e) => error!("生成消息出现异常。{}", e),
		}

		return match timeout(Duration::from_secs(3), transport.next()).await {
			Ok(Some(Ok(mut request))) => {
				return match self._inner.decode_read_message(&mut request) {
					Ok(json) => {
						if json["Status"] != 0 {
							warn!("登录出现错误。返回值:{}", json);
							Err(io::Error::new(io::ErrorKind::PermissionDenied, json.dump()))
						} else {
							Ok(transport)
						}
					}
					Err(e) => {
						warn!("连接出错: {}", e);
						Err(io::Error::new(io::ErrorKind::PermissionDenied, e))
					}
				};
			}
			Ok(Some(Err(e))) => {
				error!("这里是错误?? err = {:?}", e);
				Err(io::Error::new(io::ErrorKind::PermissionDenied, e))
			}
			Ok(None) => Err(io::Error::new(io::ErrorKind::Other, "")),
			Err(e) => {
				warn!("连接超时。退出。");
				Err(io::Error::new(io::ErrorKind::TimedOut, e))
			}
		};
	}
}
