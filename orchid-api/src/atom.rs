use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{ExprTicket, Expression, ExtHostReq, HostExtNotif, HostExtReq, OrcResult, SysId, TStrv};

pub type AtomData = Vec<u8>;

/// Unique ID associated with atoms that have an identity
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct AtomId(pub NonZeroU64);

/// An atom owned by an implied system. Usually used in responses from a system.
/// This has the same semantics as [Atom] except in that the owner is implied.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct LocalAtom {
  pub drop: Option<AtomId>,
  pub data: AtomData,
}
impl LocalAtom {
  pub fn associate(self, owner: SysId) -> Atom { Atom { owner, drop: self.drop, data: self.data } }
}

/// An atom representation that can be serialized and sent around. Atoms
/// represent the smallest increment of work.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct Atom {
  /// Instance ID of the system that created the atom
  pub owner: SysId,
  /// Indicates how the owner should be notified when this atom is dropped.
  /// Construction is always explicit and atoms are never cloned.
  ///
  /// Atoms with `drop == None` are also known as trivial, they can be
  /// duplicated and stored with no regard to expression lifetimes. NOTICE
  /// that this only applies to the atom. If it's referenced with an
  /// [ExprTicket], the ticket itself can still expire.
  ///
  /// Notice also that the atoms still expire when the system is dropped, and
  /// are not portable across instances of the same system, so this doesn't
  /// imply that the atom is serializable.
  pub drop: Option<AtomId>,
  /// Data stored in the atom. This could be a key into a map, or the raw data
  /// of the atom if it isn't too big.
  pub data: AtomData,
}

/// Attempt to apply an atom as a function to an expression
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct CallRef(pub Atom, pub ExprTicket);
impl Request for CallRef {
  type Response = Expression;
}

/// Attempt to apply an atom as a function, consuming the atom and enabling the
/// library to reuse its datastructures rather than duplicating them. This is an
/// optimization over [CallRef] followed by [AtomDrop].
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct FinalCall(pub Atom, pub ExprTicket);
impl Request for FinalCall {
  type Response = Expression;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct SerializeAtom(pub Atom);
impl Request for SerializeAtom {
  type Response = (Vec<u8>, Vec<ExprTicket>);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(HostExtReq)]
pub struct DeserAtom(pub SysId, pub Vec<u8>, pub Vec<ExprTicket>);
impl Request for DeserAtom {
  type Response = Atom;
}

/// A request blindly routed to the system that provides an atom.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct Fwded(pub Atom, pub TStrv, pub Vec<u8>);
impl Request for Fwded {
  type Response = Option<Vec<u8>>;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExtHostReq)]
pub struct Fwd(pub Atom, pub TStrv, pub Vec<u8>);
impl Request for Fwd {
  type Response = Option<Vec<u8>>;
}

#[derive(Clone, Debug, Coding)]
pub enum NextStep {
  Continue(Expression),
  Halt,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct Command(pub Atom);
impl Request for Command {
  type Response = OrcResult<NextStep>;
}

/// Notification that an atom is being dropped because its associated expression
/// isn't referenced anywhere. This should have no effect if the atom's `drop`
/// flag is false.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(HostExtNotif)]
pub struct AtomDrop(pub SysId, pub AtomId);

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(AtomReq, HostExtReq)]
pub struct AtomPrint(pub Atom);
impl Request for AtomPrint {
  type Response = String;
}

/// Requests that apply to an existing atom instance
#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(HostExtReq)]
#[extendable]
pub enum AtomReq {
  CallRef(CallRef),
  FinalCall(FinalCall),
  Fwded(Fwded),
  Command(Command),
  AtomPrint(AtomPrint),
  SerializeAtom(SerializeAtom),
}
impl AtomReq {
  /// Obtain the first [Atom] argument of the request. All requests in this
  /// subclass have at least one atom argument.
  pub fn get_atom(&self) -> &Atom {
    match self {
      Self::CallRef(CallRef(a, ..))
      | Self::Command(Command(a))
      | Self::FinalCall(FinalCall(a, ..))
      | Self::Fwded(Fwded(a, ..))
      | Self::AtomPrint(AtomPrint(a))
      | Self::SerializeAtom(SerializeAtom(a)) => a,
    }
  }
}
