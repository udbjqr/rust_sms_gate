use std::cmp::max;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use log::{error, info, warn};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use tokio::time::{Duration, sleep, timeout};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::clients::Channel::Channel;
use crate::clients::client_entity::ConnectStat::{Connect, Disconnect};
use crate::cmpp::Cmpp;
use crate::get_runtime;
use crate::protocol::{Protocol, ProtocolType};

#[derive(Debug, Clone, Copy)]
enum ConnectStat {
	Disconnect,
	Connect(usize),
}

impl std::fmt::Display for ConnectStat {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Disconnect => write!(f, "Disconnect"),
			Connect(n) => write!(f, "Connect,数量:{}", n)
		}
	}
}


#[derive(Debug)]
pub struct ClientEntity {
	addr: String,
	user_name: String,
	password: String,
	max_conn_num: usize,
	conn_num: usize,
	conn_statue: ConnectStat,
	seq: Arc<AtomicU32>,
	protocol_type: ProtocolType,
}

impl ClientEntity {
	fn get_sequence(&self) -> u32 {
		self.seq.fetch_add(1, Ordering::Relaxed)
	}

	pub fn new(addr: String, user_name: String, password: String, max_num: usize, protocol_type: ProtocolType) -> ClientEntity {
		ClientEntity {
			addr,
			user_name,
			password,
			max_conn_num: max_num,
			conn_num: 0,
			conn_statue: Disconnect,
			seq: Arc::new(AtomicU32::new(rand::random())),
			protocol_type,
		}
	}

	pub async fn start_work(&mut self, close_trigger: Receiver<String>, send_tx: Sender<String>) -> Result<(), io::Error> {
		for i in 0..self.max_conn_num {
			let addr = self.addr.clone();
			let user_name = self.user_name.clone();
			let password = self.password.clone();
			let conn_statue = Arc::new(AtomicBool::new(false));
			let conn_statue_in = Arc::new(AtomicBool::new(false)).clone();
			let seq = self.seq.clone();
			let (channel_msg_tx, channel_msg_rx) = oneshot::channel::<String>();
			let (read_msg_tx, read_msg_rx) = mpsc::channel::<String>(0xffffffff);
			let (write_msg_tx, write_msg_rx) = mpsc::channel::<String>(0xffffffff);

			let read_msg_tx = read_msg_tx.clone();

			let protocol = match &self.protocol_type {
				CMPP => Cmpp::new(),
				_ => Cmpp::new()
			};

			get_runtime().spawn(async move {
				let mut channel = Channel::new(addr, user_name, password, conn_statue, seq, channel_msg_rx, read_msg_tx, write_msg_rx, protocol);

				channel.start_work().await;
			});
		}

		//TODO 这里自己也不停下来。准备接收断开等等处理动作。。

		Ok(())
	}
}


impl Display for ClientEntity {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "addr:{},user_name:{},password:{},max_conn:{},状态:{}",
		       &self.addr,
		       &self.user_name,
		       &self.password,
		       &self.max_conn_num,
		       &self.conn_statue)
	}
}
