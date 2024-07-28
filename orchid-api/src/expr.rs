use std::num::NonZeroU64;

use orchid_api_derive::{Coding, Hierarchy};
use orchid_api_traits::Request;

use crate::atom::{Atom, LocalAtom};
use crate::error::ProjErr;
use crate::interner::TStrv;
use crate::location::Location;
use crate::proto::{ExtHostNotif, ExtHostReq};
use crate::system::SysId;

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
pub struct Relocate {
  pub dec: SysId,
  pub inc: SysId,
  pub expr: ExprTicket,
}

/// A description of a new expression. It is used as the return value of
/// [crate::atom::Call] or [crate::atom::CallRef], or a constant in the
/// [crate::tree::Tree].
#[derive(Clone, Debug, Coding)]
pub enum Clause {
  /// Apply the lhs as a function to the rhs
  Call(Box<Expr>, Box<Expr>),
  /// Lambda function. The number operates as an argument name
  Lambda(u64, Box<Expr>),
  /// Binds the argument passed to the lambda with the same ID in the same
  /// template
  Arg(u64),
  /// Insert the specified host-expression in the template here. When the clause
  /// is used in the const tree, this variant is forbidden.
  Slot(ExprTicket),
  /// The lhs must be fully processed before the rhs can be processed.
  /// Equivalent to Haskell's function of the same name
  Seq(Box<Expr>, Box<Expr>),
  /// Insert a new atom in the tree. When the clause is used in the const tree,
  /// the atom must be trivial. This is always a newly constructed atom, if you
  /// want to reference an existing atom, use the corresponding [ExprTicket].
  /// Because the atom is newly constructed, it also must belong to this system.
  /// For convenience, [SysId::MAX] is also accepted as referring to this
  /// system.
  NewAtom(LocalAtom),
  /// An atom, specifically an atom that already exists. This form is only ever
  /// returned from [Inspect], and it's borrowed from the expression being
  /// inspected.
  Atom(ExprTicket, Atom),
  /// A reference to a constant
  Const(TStrv),
  /// A static runtime error.
  Bottom(Vec<ProjErr>),
}

#[derive(Clone, Debug, Coding)]
pub struct Expr {
  pub clause: Clause,
  pub location: Location,
}

#[derive(Clone, Debug, Coding)]
pub struct Details {
  pub expr: Expr,
  pub refcount: u32,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Coding, Hierarchy)]
#[extends(ExprReq, ExtHostReq)]
pub struct Inspect(pub ExprTicket);
impl Request for Inspect {
  type Response = Details;
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
  Relocate(Relocate),
}
