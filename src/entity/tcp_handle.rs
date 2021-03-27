use json::JsonValue;
use tokio::sync::mpsc;

pub async fn handle_rev_msg(msg: JsonValue, tx: mpsc::Sender<JsonValue>) {
	//TODO 进行收到消息的处理
	println!("{:?}", msg)
}
