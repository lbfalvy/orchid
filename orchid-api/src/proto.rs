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

use std::io::{Read, Write};

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::{read_exact, write_exact, Channel, Decode, Encode, MsgSet, Request};

use crate::logging::{self, LogStrategy};
use crate::{atom, expr, interner, parser, system, tree, vfs};

static HOST_INTRO: &[u8] = b"Orchid host, binary API v0\n";
pub struct HostHeader {
  pub log_strategy: LogStrategy,
}
impl Decode for HostHeader {
  fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
    read_exact(read, HOST_INTRO);
    Self { log_strategy: LogStrategy::decode(read) }
  }
}
impl Encode for HostHeader {
  fn encode<W: Write + ?Sized>(&self, write: &mut W) {
    write_exact(write, HOST_INTRO);
    self.log_strategy.encode(write)
  }
}

static EXT_INTRO: &[u8] = b"Orchid extension, binary API v0\n";
pub struct ExtensionHeader {
  pub name: String,
  pub systems: Vec<system::SystemDecl>,
}
impl Decode for ExtensionHeader {
  fn decode<R: Read + ?Sized>(read: &mut R) -> Self {
    read_exact(read, EXT_INTRO);
    Self { name: String::decode(read), systems: Vec::decode(read) }
  }
}
impl Encode for ExtensionHeader {
  fn encode<W: Write + ?Sized>(&self, write: &mut W) {
    write_exact(write, EXT_INTRO);
    self.name.encode(write);
    self.systems.encode(write)
  }
}

#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct Ping;
impl Request for Ping {
  type Response = ();
}

/// Requests running from the extension to the host
#[derive(Clone, Coding, Hierarchy)]
#[extendable]
pub enum ExtHostReq {
  Ping(Ping),
  IntReq(interner::IntReq),
  Fwd(atom::Fwd),
  ExprReq(expr::ExprReq),
  SubLex(parser::SubLex),
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
  ParserReq(parser::ParserReq),
  GetMember(tree::GetMember),
  VfsReq(vfs::VfsReq),
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
  use system::{SysDeclId, SystemDecl};

  use super::*;

  #[test]
  fn host_header_enc() {
    let hh = HostHeader { log_strategy: LogStrategy::File("SomeFile".to_string()) };
    let mut enc = &enc_vec(&hh)[..];
    eprintln!("Encoded to {enc:?}");
    HostHeader::decode(&mut enc);
    assert_eq!(enc, []);
  }

  #[test]
  fn ext_header_enc() {
    let eh = ExtensionHeader {
      name: "my_extension".to_string(),
      systems: vec![SystemDecl {
        id: SysDeclId(1.try_into().unwrap()),
        name: "misc".to_string(),
        depends: vec!["std".to_string()],
        priority: NotNan::new(1f64).unwrap(),
      }],
    };
    let mut enc = &enc_vec(&eh)[..];
    eprintln!("Encoded to {enc:?}");
    ExtensionHeader::decode(&mut enc);
    assert_eq!(enc, [])
  }
}
