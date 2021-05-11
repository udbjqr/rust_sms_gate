# rust_sms_gate
- 一个使用rust进行开发的短信网关,三网合一短信,支持CMPP\SMGP\SGIP协议.
- 使用了tokio做为异步的底层.所以发送和接收效率非常高.
- 此项目仅是一个网关,负责接收和发送短信..
- 使用kafka做为消息转发.所以此系统要启动,需要配置config/message_receiver.json文件.

#How to use

# kafka的配置
## 网关接收的消息
### 短信
- send.submit 需要发送的短信内容
- send.deliver 需要发送的上行短信内容
- send.report 需要发送的状态报告内容

### 通道
- passage.add 新增加一个服务商通道
- passage.modify 对一个服务商通道进行修改
- passage.remove 移除一个通道
- passage.request.state 接收需要当前通道状态修改的请求

### 客户
- account.add 新增加一个客户
- account.modify 对一个客户进行修改
- account.remove 移除一个客户

## 网关发送的消息

### 短信
- sms.send.return.failure 短信发送失败消息.
- toB.submit 接收到短信发送请求向外发送.
- toB.submit.response 接收到短信请求复向外发送
- toB.deliver 接收到上行短信请求向外发送
- toB.deliver.response 接收到上行短信回复和状态报告回复向外发送.
- toB.report 接收到状态报告向外发送

### 通道
- passage.state.change 当连接状态发生变化时发送此消息

### 客户
- account.state.change 当连接状态发生变化时发送此消息

### 其他
- lower.computer.init 网关启动后会发送此消息.希望获取通道\客户等的初始化消息

# 监听端口
- 编辑confing/smsServer.json文件,设置监听的端口
