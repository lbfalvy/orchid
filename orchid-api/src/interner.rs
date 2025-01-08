use std::num::NonZeroU64;
use std::sync::Arc;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{ExtHostReq, HostExtReq};

/// Intern requests sent by the replica to the master. These requests are
/// repeatable.
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostReq)]
#[extendable]
pub enum IntReq {
	InternStr(InternStr),
	InternStrv(InternStrv),
	ExternStr(ExternStr),
	ExternStrv(ExternStrv),
}

/// replica -> master to intern a string on the master. Repeatable.
///
/// See [IntReq]
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(IntReq, ExtHostReq)]
pub struct InternStr(pub Arc<String>);
impl Request for InternStr {
	type Response = TStr;
}

/// replica -> master to find the interned string corresponding to a key.
///
/// Repeatable.
///
/// See [IntReq]
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(IntReq, ExtHostReq)]
pub struct ExternStr(pub TStr);
impl Request for ExternStr {
	type Response = Arc<String>;
}
/// replica -> master to intern a vector of interned strings
///
/// Repeatable.
///
/// See [IntReq]
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(IntReq, ExtHostReq)]
pub struct InternStrv(pub Arc<Vec<TStr>>);
impl Request for InternStrv {
	type Response = TStrv;
}
/// replica -> master to find the vector of interned strings corresponding to a
/// token
///
/// Repeatable.
///
/// See [IntReq]
#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(IntReq, ExtHostReq)]
pub struct ExternStrv(pub TStrv);
impl Request for ExternStrv {
	type Response = Arc<Vec<TStr>>;
}

/// A substitute for an interned string in serialized datastructures.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct TStr(pub NonZeroU64);

/// A substitute for an interned string sequence in serialized datastructures.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct TStrv(pub NonZeroU64);

/// A request to sweep the replica. The master will not be sweeped until all
/// replicas respond, as it must retain everything the replicas retained
#[derive(Clone, Copy, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct Sweep;
impl Request for Sweep {
	type Response = Retained;
}

/// List of keys in this replica that couldn't be sweeped because local
/// datastructures reference their value.
#[derive(Clone, Debug, Coding)]
pub struct Retained {
	pub strings: Vec<TStr>,
	pub vecs: Vec<TStrv>,
}
