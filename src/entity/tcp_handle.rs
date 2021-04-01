use json::JsonValue;

pub async fn handle_rev_msg(msg: JsonValue) {
	//TODO 进行收到消息的处理
	//TODO 直接向消息队列发送。不需要再拿对象
	//TODO 根据不同格式。发不同消息队列
	println!("{:?}", msg)
}
