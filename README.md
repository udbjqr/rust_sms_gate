# rust_sms_gate
- 一个使用rust进行开发的短信网关,三网合一短信,支持CMPP\SMGP\SGIP协议.
- 使用了tokio做为异步的底层.所以发送和接收效率非常高.
- 此项目是一个网关,负责接收和发送短信，并且将相关业务逻辑发送至消息队列。
- 使用kafka做为消息转发.所以此系统要启动,需要配置config/message_receiver.json文件.

# How to use

# kafka的配置
## 网关接收并处理的消息主题
### 短信
- send.submit 需要发送的短信内容
  ```json
  {
    "id": 12, 
    "is_priority": false, 
    "msg_fmt": 0,
    "src_id": "10680000",
    "dest_ids": ["1811234567"],
    "msg_content": "test",
    "at_time": "",
    "valid_time": "",
    "msg_ids": ["0720102545000693402291"],
    "msg_type": "Submit"
  }
  ```
  - id: 指明发送时使用的通道id。
  - is_priority: 指明发送时是否使用优先通道，
  - msg_fmt: 消息内容的格式。具体见相关协议
  - src_id： 发送的主叫号码
  - dest_ids: 短信的被叫号码
  - msg_content: 短信的内容，长短信会自动拆分
  - at_time: 具体见相关协议
  - valid_time: 具体见相关协议
  - msg_ids: 对应每一条短信的msg_id.此值不会被发送出去，但收到回执时会和收到的msg_id一同发回。可做为单条短信的唯一标识
  - msg_type: 发送类型
  
- send.deliver 需要发送的上行短信内容
  ```json
  {
    "id":4,
    "msg_fmt":15,
    "src_id":"17779522835",
    "msg_content":"12",
    "passage_id":11,
    "msg_type":"Deliver",
    "dest_id":"10683074193009",
    "msg_id":"05700707211558527519"
  }
  ```
  - id: 指明发送时使用的通道id。
  - msg_fmt: 消息内容的格式。具体见相关协议
  - src_id： 发送的主叫号码
  - dest_ids: 短信的被叫号码
  - msg_content: 短信的内容，长短信会自动拆分
  - msg_id: 对应每一条短信的msg_id.此值不会被发送出去，但收到回执时会和收到的msg_id一同发回。可做为单条短信的唯一标识

- send.report 需要发送的状态报告内容
  ```json
  {
    "id":4,
    "is_priority":false,
    "msg_fmt":0,
    "src_id":"1801234567",
    "submit_time":"2107211906",
    "msg_type":"Report",
    "state":"DELIVRD",
    "dest_id":"1068301234560",
    "msg_id":"072119060796238135106",
    "done_time":"2107211906"
  }
  ```
  - id: 指明发送时使用的通道id。
  - is_priority: 指明发送时是否使用优先通道，
  - msg_fmt: 消息内容的格式。具体见相关协议
  - src_id： 短信的收到号码
  - dest_ids: 短信的发送号码
  - msg_id: 对应每一条短信的msg_id.此值对应submit里面的msg_id

### 通道
- passage.add 新增加一个服务商通道
   ```json
  {
  "extAccessCode":"10682247",
  "flag":1,
  "op_name":"",
  "gatewayIp":"127.0.0.1:9002",
  "protocolType":"SMGP",
  "isLongContent":"1",
  "type":"PASSAGE_CHINA_TELECOM",
  "spId":"101094",
  "connNum":1,
  "gatewayPort":"9002",
  "gatewayAutoType":"1",
  "op_code":"DXZL1004",
  "password":"123456",
  "isMms":"0",
  "readLimit":50,
  "smsAutoPlace":"1",
  "loginName":"101094",
  "writeLimit":50,
  "id":12,
  "serviceId":"31170036640006"
  }
  ```
  - extAccessCode: 通道
- passage.modify 对一个服务商通道进行修改
- passage.remove 移除一个通道
- passage.request.state 接收需要当前通道状态修改的请求

### 客户
- account.add 新增加一个客户
- account.modify 对一个客户进行修改
- account.remove 移除一个客户

## 网关发送的消息主题
### 短信
- sms.send.return.failure 短信发送失败消息.
- toB.submit 接收到短信发送请求向外发送.
- toB.submit.response 接收到短信请求复向外发送
- toB.deliver 接收到上行短信请求向外发送
  ```json
  {
    "msg_id":"05700707200922548597",
    "msg_fmt":15,
    "r_t":"20210719191731",
    "src_id":"181123457899",
    "dest_id":"1068111111111111",
    "msg_content":"这是一个测试，这个也能包含长短信",
    "msg_type":"Deliver",
    "receive_time":1626744148,
    "entity_id":11
  }
  ```
  - msg_id： 收到上行的msg_id
  - msg_fmt： 内容格式
  - r_t： 接收时间
  - src_id： 短信发送方号码 
  - dest_id： 短信接收方号码
  - msg_content： 短信内容
  - msg_type： 消息类型
  - receive_time： 接收时间的long格式
  - entity_id： 对应的通道的id,
  
- toB.deliver.response 接收到上行短信回复和状态报告回复向外发送.
  ```json
  {
    "m_t":2147483653,
    "msg_id":"050607072115585207519",
    "result":0,
    "msg_type":"DeliverResp",
    "receive_time":1626854317,
    "entity_id":4,
    "account_msg_id":"05700707211558527519"
  }
  ```
  - msg_id：当前收到上行回复的msg_id
  - account_msg_id：对应submit的msg_id
- toB.report 接收到状态报告向外发送
    ```json
  {
    "msg_id":"05700807211906736468",
    "msg_fmt":0,
    "r_t":"20210721190609",
    "src_id":"18019858527",
    "dest_id":"10683074193009300940",
    "is_report":true,
    "submit_time":"2107211906",
    "done_time":"2107211906",
    "state":"DELIVRD",
    "error_code":"000",
    "msg_type":"Report",
    "receive_time":1626865569,
    "entity_id":11
  }
  ```

### 通道
- passage.state.change 当连接状态发生变化时发送此消息

### 客户
- account.state.change 当连接状态发生变化时发送此消息

### 其他
- lower.computer.init 网关启动后会发送此消息.希望获取通道\客户等的初始化消息

# 监听端口
- 编辑confing/smsServer.json文件,设置监听的端口 可设置CMPP\SMGP\SGIP协议的相关端口。每种协议一个。不可多设置
