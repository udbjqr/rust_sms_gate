use std::fmt::{Display, Debug};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub enum MsgType {
	Connect,
	ConnectResp,
	Terminate,
	TerminateResp,
	Submit,
	SubmitResp,
	Deliver,
	DeliverResp,
	Report,
	ReportResp,
	Query,
	QueryResp,
	Cancel,
	CancelResp,
	ActiveTest,
	ActiveTestResp,
	Fwd,
	FwdResp,
	MtRoute,
	MtRouteResp,
	MoRoute,
	MoRouteResp,
	GetMtRoute,
	GetMtRouteResp,
	MtRouteUpdate,
	MtRouteUpdateResp,
	MoRouteUpdate,
	MoRouteUpdateResp,
	PushMtRouteUpdate,
	PushMtRouteUpdateResp,
	PushMoRouteUpdate,
	PushMoRouteUpdateResp,
	GetMoRoute,
	GetMoRouteResp,
	UnKnow,
}


///协议的错误回复
#[derive(Debug, Clone, Copy)]
pub enum SmsStatus<T> {
	///成功。
	Success(T),
	///消息结构错
	MessageError(T),
	///非法源地址
	AddError(T),
	///认证错
	AuthError(T),
	///版本太高
	VersionError(T),
	///登录时的其他错误
	LoginOtherError(T),
	///流量限制
	TrafficRestrictions(T)
}

impl<T> std::error::Error for SmsStatus<T> where T: Display + Debug {}

impl<T> fmt::Display for SmsStatus<T>
	where T: Display {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			SmsStatus::Success(v) => write!(f, "成功,{}", v),
			SmsStatus::MessageError(v) => write!(f, "登录,消息结构错,{}",v),
			SmsStatus::AddError(v) => write!(f, "登录,非法源地址,{}",v),
			SmsStatus::AuthError(v) => write!(f, "登录,认证错,{}",v),
			SmsStatus::VersionError(v) => write!(f, "登录,版本太高,{}",v),
			SmsStatus::LoginOtherError(v) => write!(f, "登录,其他错误,{}", v),
			SmsStatus::TrafficRestrictions(v) => write!(f, "发送.流量限制,{}",v),
			// _ => write!(f, "其他错误,这里没有更新。"),
		}
	}
}
