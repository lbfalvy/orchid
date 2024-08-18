use std::collections::HashMap;
use std::num::NonZeroU16;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::error::OrcResult;
use crate::interner::TStr;
use crate::proto::HostExtReq;
use crate::system::SysId;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct VfsId(pub NonZeroU16);

#[derive(Clone, Debug, Coding)]
pub enum Loaded {
  Code(String),
  Collection(Vec<TStr>),
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(VfsReq, HostExtReq)]
pub struct VfsRead(pub SysId, pub VfsId, pub Vec<TStr>);
impl Request for VfsRead {
  type Response = OrcResult<Loaded>;
}

#[derive(Clone, Debug, Coding)]
pub enum EagerVfs {
  Lazy(VfsId),
  Eager(HashMap<TStr, EagerVfs>),
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(VfsReq, HostExtReq)]
pub struct GetVfs(pub SysId);
impl Request for GetVfs {
  type Response = EagerVfs;
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(HostExtReq)]
#[extendable]
pub enum VfsReq {
  GetVfs(GetVfs),
  VfsRead(VfsRead),
}
