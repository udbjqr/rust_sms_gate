use std::fmt::{Debug};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
	UNKNOWN,
}

impl From<&str> for MsgType {
	fn from(name: &str) -> Self {
		match name {
			"Submit" => MsgType::Submit,
			"SubmitResp" => MsgType::SubmitResp,
			"Deliver" => MsgType::Deliver,
			"DeliverResp" => MsgType::DeliverResp,
			"Report" => MsgType::Report,
			"ReportResp" => MsgType::ReportResp,
			"Connect" => MsgType::Connect,
			"ConnectResp" => MsgType::ConnectResp,
			"Terminate" => MsgType::Terminate,
			"TerminateResp" => MsgType::TerminateResp,
			"Query" => MsgType::Query,
			"QueryResp" => MsgType::QueryResp,
			"Cancel" => MsgType::Cancel,
			"CancelResp" => MsgType::CancelResp,
			"ActiveTest" => MsgType::ActiveTest,
			"ActiveTestResp" => MsgType::ActiveTestResp,
			"Fwd" => MsgType::Fwd,
			"FwdResp" => MsgType::FwdResp,
			"MtRoute" => MsgType::MtRoute,
			"MtRouteResp" => MsgType::MtRouteResp,
			"MoRoute" => MsgType::MoRoute,
			"MoRouteResp" => MsgType::MoRouteResp,
			"GetMtRoute" => MsgType::GetMtRoute,
			"GetMtRouteResp" => MsgType::GetMtRouteResp,
			"MtRouteUpdate" => MsgType::MtRouteUpdate,
			"MtRouteUpdateResp" => MsgType::MtRouteUpdateResp,
			"MoRouteUpdate" => MsgType::MoRouteUpdate,
			"MoRouteUpdateResp" => MsgType::MoRouteUpdateResp,
			"PushMtRouteUpdate" => MsgType::PushMtRouteUpdate,
			"PushMtRouteUpdateResp" => MsgType::PushMtRouteUpdateResp,
			"PushMoRouteUpdate" => MsgType::PushMoRouteUpdate,
			"PushMoRouteUpdateResp" => MsgType::PushMoRouteUpdateResp,
			"GetMoRoute" => MsgType::GetMoRoute,
			"GetMoRouteResp" => MsgType::GetMoRouteResp,
			_ => MsgType::UNKNOWN,
		}
	}
}

impl Into<&str> for MsgType {
	fn into(self) -> &'static str {
		match self {
			MsgType::Submit => "Submit",
			MsgType::SubmitResp => "SubmitResp",
			MsgType::Deliver => "Deliver",
			MsgType::DeliverResp => "DeliverResp",
			MsgType::Report => "Report",
			MsgType::ReportResp => "ReportResp",
			MsgType::Connect => "Connect",
			MsgType::ConnectResp => "ConnectResp",
			MsgType::Terminate => "Terminate",
			MsgType::TerminateResp => "TerminateResp",
			MsgType::Query => "Query",
			MsgType::QueryResp => "QueryResp",
			MsgType::Cancel => "Cancel",
			MsgType::CancelResp => "CancelResp",
			MsgType::ActiveTest => "ActiveTest",
			MsgType::ActiveTestResp => "ActiveTestResp",
			MsgType::Fwd => "Fwd",
			MsgType::FwdResp => "FwdResp",
			MsgType::MtRoute => "MtRoute",
			MsgType::MtRouteResp => "MtRouteResp",
			MsgType::MoRoute => "MoRoute",
			MsgType::MoRouteResp => "MoRouteResp",
			MsgType::GetMtRoute => "GetMtRoute",
			MsgType::GetMtRouteResp => "GetMtRouteResp",
			MsgType::MtRouteUpdate => "MtRouteUpdate",
			MsgType::MtRouteUpdateResp => "MtRouteUpdateResp",
			MsgType::MoRouteUpdate => "MoRouteUpdate",
			MsgType::MoRouteUpdateResp => "MoRouteUpdateResp",
			MsgType::PushMtRouteUpdate => "PushMtRouteUpdate",
			MsgType::PushMtRouteUpdateResp => "PushMtRouteUpdateResp",
			MsgType::PushMoRouteUpdate => "PushMoRouteUpdate",
			MsgType::PushMoRouteUpdateResp => "PushMoRouteUpdateResp",
			MsgType::GetMoRoute => "GetMoRoute",
			MsgType::GetMoRouteResp => "GetMoRouteResp",
			MsgType::UNKNOWN => "UnKnow",
		}
	}
}

///协议的错误回复
#[derive(Debug, Clone, Copy)]
pub enum SmsStatus {
	///成功。
	Success,
	///消息结构错
	MessageError,
	///非法源地址
	AddError,
	///认证错
	AuthError,
	///版本太高
	VersionError,
	///登录时的其他错误
	OtherError,
	///流量限制
	TrafficRestrictions,
	UNKNOWN,
}

impl From<&'static str> for SmsStatus {
	fn from(name: &'static str) -> Self {
		match name {
			"Success" => SmsStatus::Success,
			"MessageError" => SmsStatus::MessageError,
			"AddError" => SmsStatus::AddError,
			"AuthError" => SmsStatus::AuthError,
			"VersionError" => SmsStatus::VersionError,
			"LoginOtherError" => SmsStatus::OtherError,
			"TrafficRestrictions" => SmsStatus::TrafficRestrictions,
			_ => SmsStatus::UNKNOWN
		}
	}
}


impl Into<&'static str> for SmsStatus {
	fn into(self) -> &'static str {
		match self {
			SmsStatus::Success => "Success",
			SmsStatus::MessageError => "MessageError",
			SmsStatus::AddError => "AddError",
			SmsStatus::AuthError => "AuthError",
			SmsStatus::VersionError => "VersionError",
			SmsStatus::OtherError => "LoginOtherError",
			SmsStatus::TrafficRestrictions => "TrafficRestrictions",
			SmsStatus::UNKNOWN => "UNKNOWN",
		}
	}
}

impl std::error::Error for SmsStatus {}

impl fmt::Display for SmsStatus {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			SmsStatus::Success => write!(f, "成功"),
			SmsStatus::MessageError => write!(f, "登录,消息结构错"),
			SmsStatus::AddError => write!(f, "登录,非法源地址,"),
			SmsStatus::AuthError => write!(f, "登录,认证错,"),
			SmsStatus::VersionError => write!(f, "登录,版本太高,"),
			SmsStatus::OtherError => write!(f, "登录,其他错误"),
			SmsStatus::TrafficRestrictions => write!(f, "发送.流量限制"),
			SmsStatus::UNKNOWN => write!(f, "未知的错误."),
			// _ => write!(f, "其他错误,这里没有更新。"),
		}
	}
}