//! Basic messages of the Orchid extension API.
//!
//! The protocol is defined over a byte stream, normally the stdin/stdout of the
//! extension. The implementations of [Coding] in this library are considered
//! normative. Any breaking change here or in the default implementations of
//! [Coding] must also increment the version number in the intro strings.
//!
//! 3 different kinds of messages are recognized; request, response, and
//! notification. There are no general ordering guarantees about these, multiple
//! requests, even requests of the same type may be sent concurrently, unless
//! otherwise specified in the request's definition.
//!
//! Each message begins with a u32 length, followed by that many bytes of
//! message content. The first byte of the content is a u64 combined request ID
//! and discriminator, D.
//!
//! - If D = 0, the rest of the content is a notification.
//! - If 0 < D < 2^63, it is a request with identifier D.
//! - If 2^63 <= D, it is a response to request identifier !D.
//!
//! The order of both notifications and requests sent from the same thread must
//! be preserved. Toolkits must ensure that the client code is able to observe
//! the ordering of messages.

use std::pin::Pin;

use async_std::io::{Read, Write};
use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::{Channel, Decode, Encode, MsgSet, Request, read_exact, write_exact};

use crate::{atom, expr, interner, lexer, logging, macros, parser, system, tree, vfs};

static HOST_INTRO: &[u8] = b"Orchid host, binary API v0\n";
pub struct HostHeader {
	pub log_strategy: logging::LogStrategy,
}
impl Decode for HostHeader {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		read_exact(read.as_mut(), HOST_INTRO).await;
		Self { log_strategy: logging::LogStrategy::decode(read).await }
	}
}
impl Encode for HostHeader {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		write_exact(write.as_mut(), HOST_INTRO).await;
		self.log_strategy.encode(write).await
	}
}

static EXT_INTRO: &[u8] = b"Orchid extension, binary API v0\n";
pub struct ExtensionHeader {
	pub name: String,
	pub systems: Vec<system::SystemDecl>,
}
impl Decode for ExtensionHeader {
	async fn decode<R: Read + ?Sized>(mut read: Pin<&mut R>) -> Self {
		read_exact(read.as_mut(), EXT_INTRO).await;
		Self { name: String::decode(read.as_mut()).await, systems: Vec::decode(read).await }
	}
}
impl Encode for ExtensionHeader {
	async fn encode<W: Write + ?Sized>(&self, mut write: Pin<&mut W>) {
		write_exact(write.as_mut(), EXT_INTRO).await;
		self.name.encode(write.as_mut()).await;
		self.systems.encode(write).await
	}
}

#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct Ping;
impl Request for Ping {
	type Response = ();
}

/// Requests running from the extension to the host
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extendable]
pub enum ExtHostReq {
	Ping(Ping),
	IntReq(interner::IntReq),
	Fwd(atom::Fwd),
	ExtAtomPrint(atom::ExtAtomPrint),
	SysFwd(system::SysFwd),
	ExprReq(expr::ExprReq),
	SubLex(lexer::SubLex),
	RunMacros(macros::RunMacros),
}

/// Notifications sent from the extension to the host
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Coding, Hierarchy)]
#[extendable]
pub enum ExtHostNotif {
	ExprNotif(expr::ExprNotif),
	Log(logging::Log),
}

pub struct ExtHostChannel;
impl Channel for ExtHostChannel {
	type Notif = ExtHostNotif;
	type Req = ExtHostReq;
}

/// Requests running from the host to the extension
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extendable]
pub enum HostExtReq {
	Ping(Ping),
	SysReq(system::SysReq),
	Sweep(interner::Sweep),
	AtomReq(atom::AtomReq),
	DeserAtom(atom::DeserAtom),
	LexExpr(lexer::LexExpr),
	ParseLine(parser::ParseLine),
	GetMember(tree::GetMember),
	VfsReq(vfs::VfsReq),
	ApplyMacro(macros::ApplyMacro),
}

/// Notifications sent from the host to the extension
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extendable]
pub enum HostExtNotif {
	SystemDrop(system::SystemDrop),
	AtomDrop(atom::AtomDrop),
	/// The host can assume that after this notif is sent, a correctly written
	/// extension will eventually exit.
	Exit,
}

pub struct HostExtChannel;
impl Channel for HostExtChannel {
	type Notif = HostExtNotif;
	type Req = HostExtReq;
}

/// Message set viewed from the extension's perspective
pub struct ExtMsgSet;
impl MsgSet for ExtMsgSet {
	type In = HostExtChannel;
	type Out = ExtHostChannel;
}

/// Message Set viewed from the host's perspective
pub struct HostMsgSet;
impl MsgSet for HostMsgSet {
	type In = ExtHostChannel;
	type Out = HostExtChannel;
}

#[cfg(test)]
mod tests {
	use orchid_api_traits::enc_vec;
	use ordered_float::NotNan;
	use test_executors::spin_on;

	use super::*;

	#[test]
	fn host_header_enc() {
		spin_on(async {
			let hh = HostHeader { log_strategy: logging::LogStrategy::File("SomeFile".to_string()) };
			let mut enc = &enc_vec(&hh).await[..];
			eprintln!("Encoded to {enc:?}");
			HostHeader::decode(Pin::new(&mut enc)).await;
			assert_eq!(enc, []);
		})
	}

	#[test]
	fn ext_header_enc() {
		spin_on(async {
			let eh = ExtensionHeader {
				name: "my_extension".to_string(),
				systems: vec![system::SystemDecl {
					id: system::SysDeclId(1.try_into().unwrap()),
					name: "misc".to_string(),
					depends: vec!["std".to_string()],
					priority: NotNan::new(1f64).unwrap(),
				}],
			};
			let mut enc = &enc_vec(&eh).await[..];
			eprintln!("Encoded to {enc:?}");
			ExtensionHeader::decode(Pin::new(&mut enc)).await;
			assert_eq!(enc, [])
		})
	}
}
