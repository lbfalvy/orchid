use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::{Atom, ExtHostNotif, ExtHostReq, Location, OrcError, SysId, TStrv};

/// An arbitrary ID associated with an expression on the host side. Incoming
/// tickets always come with some lifetime guarantee, which can be extended with
/// [Acquire].
///
/// The ID is globally unique within its lifetime, but may be reused.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Coding)]
pub struct ExprTicket(pub NonZeroU64);

/// Acquire a strong reference to an expression. This keeps it alive until a
/// corresponding [Release] is emitted. The number of times a system has
/// acquired an expression is counted, and it is the system's responsibility to
/// ensure that acquires and releases pair up. Behaviour in case of a
/// superfluous free is not defined.
///
/// Some contexts may specify that an ingress [ExprTicket] is owned, this means
/// that acquiring it is not necessary.
///
/// This can be called with a foreign system to signal that an owned reference
/// is being passed, though [Relocate] may be a better fit.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExprNotif, ExtHostNotif)]
pub struct Acquire(pub SysId, pub ExprTicket);

/// Release a reference either previously acquired through either [Acquire]
/// or by receiving an owned reference. The number of times a system has
/// acquired an expression is counted, and it is the system's responsibility to
/// ensure that acquires and releases pair up. Behaviour in case of excessive
/// freeing is not defined.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExprNotif, ExtHostNotif)]
pub struct Release(pub SysId, pub ExprTicket);

/// Decrement the reference count for one system and increment it for another,
/// to indicate passing an owned reference. Equivalent to [Acquire] followed by
/// [Release].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExprNotif, ExtHostNotif)]
pub struct Move {
  pub dec: SysId,
  pub inc: SysId,
  pub expr: ExprTicket,
}

/// A description of a new expression. It is used as the return value of
/// [crate::atom::Call] or [crate::atom::CallRef], or a constant in the
/// [crate::tree::Tree].
#[derive(Clone, Debug, Coding)]
pub enum ExpressionKind {
  /// Apply the lhs as a function to the rhs
  Call(Box<Expression>, Box<Expression>),
  /// Lambda function. The number operates as an argument name
  Lambda(u64, Box<Expression>),
  /// Binds the argument passed to the lambda with the same ID in the same
  /// template
  Arg(u64),
  /// Insert the specified host-expression in the template here. When the clause
  /// is used in the const tree, this variant is forbidden.
  Slot(ExprTicket),
  /// The lhs must be fully processed before the rhs can be processed.
  /// Equivalent to Haskell's function of the same name
  Seq(Box<Expression>, Box<Expression>),
  /// Insert a new atom in the tree. When the clause is used in the const tree,
  /// the atom must be trivial. This is always a newly constructed atom, if you
  /// want to reference an existing atom, use the corresponding [ExprTicket].
  /// Because the atom is newly constructed, it also must belong to this system.
  NewAtom(Atom),
  /// A reference to a constant
  Const(TStrv),
  /// A static runtime error.
  Bottom(Vec<OrcError>),
}

#[derive(Clone, Debug, Coding)]
pub struct Expression {
  pub kind: ExpressionKind,
  pub location: Location,
}

#[derive(Clone, Debug, Coding)]
pub enum InspectedKind {
  Atom(Atom),
  Bottom(Vec<OrcError>),
  Opaque,
}

#[derive(Clone, Debug, Coding)]
pub struct Inspected {
  pub kind: InspectedKind,
  pub location: Location,
  pub refcount: u32,
}

/// Obtain information about an expression. Used to act upon arguments by
/// resolving shallowly and operating on the atom, but also usable for
/// reflection
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExprReq, ExtHostReq)]
pub struct Inspect {
  pub target: ExprTicket,
}
impl Request for Inspect {
  type Response = Inspected;
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExtHostReq)]
#[extendable]
pub enum ExprReq {
  Inspect(Inspect),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExtHostNotif)]
#[extendable]
pub enum ExprNotif {
  Acquire(Acquire),
  Release(Release),
  Move(Move),
}
