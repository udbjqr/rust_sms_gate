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
	MessageError,
	///非法源地址
	AddError,
	///认证错
	AuthError,
	///版本太高
	VersionError,
	///其他错误
	OtherError,
}

impl<T> std::error::Error for SmsStatus<T> where T: Display + Debug {}

impl<T> fmt::Display for SmsStatus<T>
	where T: Display {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			SmsStatus::Success(e) => write!(f, "成功,{}", e),
			SmsStatus::MessageError => write!(f, "登录,消息结构错"),
			SmsStatus::AddError => write!(f, "登录,非法源地址"),
			SmsStatus::AuthError => write!(f, "登录,认证错"),
			SmsStatus::VersionError => write!(f, "登录,版本太高"),
			SmsStatus::OtherError => write!(f, "登录,其他错误"),
			// _ => write!(f, "其他错误,这里没有更新。"),
		}
	}
}
