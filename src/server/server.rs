// use std::{fmt, io};
// use std::error::Error;
// use std::fmt::{Display, Formatter};
// use std::sync::{Arc, Mutex};
//
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
// use tokio::net::{TcpListener, TcpStream};
//
// use crate::runtime::get_runtime;
//
// #[derive(Debug)]
// pub struct Server {
// 	port: u32,
// 	state: Mutex<i8>,
// }
//
//
// impl Display for Server {
// 	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
// 		write!(f, "port:{} ,stat:{:?}", self.port, self.state)?;
//
// 		Ok(())
// 	}
// }
//
//
// impl Server {
// 	pub fn new(port: u32) -> Server {
// 		Server {
// 			port,
// 			state: Mutex::new(1),
// 		}
// 	}
//
// 	pub fn change_state(&self, state: i8) -> i8 {
// 		let mut num = self.state.lock().unwrap();
// 		*num = state;
//
// 		*num
// 	}
//
// 	pub fn get_state(&self) -> i8 {
// 		let mut num = self.state.lock().unwrap();
//
// 		*num
// 	}
//
//
// 	pub async fn start(server: Arc<Server>) {
// 		let run = get_runtime();
//
// 		run.spawn(async move {
// 			let listener = start_listen(server.port).await.unwrap();
//
// 			loop {
// 				let (mut socket, add) = listener.accept().await.unwrap();
// 				println!("{}连接上来? add:{}", std::thread::current().name().unwrap(), add);
//
// 				let mut buf = [0; 1024];
//
// 				loop {
// 					let n = match socket.read(&mut buf).await {
// 						// socket closed
// 						Ok(0) => return,
// 						Ok(n) => {
// 							let state = server.change_state(n as i8);
// 							println!("当前状态值:{}", server.get_state());
//
// 							n
// 						}
// 						Err(e) => {
// 							eprintln!("failed to read from socket; err = {:?}", e);
// 							return;
// 						}
// 					};
// 					println!("{}接收到数据。 buf:{:?}", std::thread::current().name().unwrap(), &buf[0..n]);
//
// 					// Write the data back
// 					if let Err(e) = socket.write_all(&buf[0..n]).await {
// 						eprintln!("failed to write to socket; err = {:?}", e);
// 						return;
// 					}
// 				}
// 			}
// 		});
// 	}
// }
//
// async fn start_listen(port: u32) -> io::Result<TcpListener> {
// 	let ip = "0.0.0.0:".to_owned() + &port.to_string();
// 	println!("开始开放端口连接,ip:{}", ip);
// 	let lis = TcpListener::bind(ip).await?;
//
// 	Ok(lis)
// }
