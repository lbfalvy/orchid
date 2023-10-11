use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

use ordered_float::NotNan;

use super::atom::StrictEq;
use super::{AtomicResult, AtomicReturn, ExternError};
use crate::error::AssertionError;
#[allow(unused)] // for doc
// use crate::define_fn;
use crate::foreign::Atomic;
use crate::interpreted::{Clause, Expr, ExprInst, TryFromExprInst};
use crate::interpreter::Context;
use crate::systems::stl::Numeric;
use crate::utils::ddispatch::{Request, Responder};

/// A proxy trait that implements [Atomic] for blobs of data in Rust code that
/// cannot be processed and always report inert. Since these are expected to be
/// parameters of functions defined with [define_fn] it also automatically
/// implements [TryFromExprInst] so that a conversion doesn't have to be
/// provided in argument lists.
pub trait InertAtomic: Debug + Clone + Send + 'static {
  /// Typename to be shown in the error when a conversion from [ExprInst] fails
  #[must_use]
  fn type_str() -> &'static str;
  /// Proxies to [Responder] so that you don't have to implmeent it manually if
  /// you need it, but behaves exactly as the default implementation.
  #[allow(unused_mut, unused_variables)] // definition should show likely usage
  fn respond(&self, mut request: Request) {}
  /// Equality comparison used by the pattern matcher. Since the pattern matcher
  /// only works with parsed code, you only need to implement this if your type
  /// is directly parseable.
  ///
  /// If your type implements [PartialEq], this can simply be implemented as
  /// ```ignore
  /// fn strict_eq(&self, other: &Self) -> bool { self == other }
  /// ```
  fn strict_eq(&self, _: &Self) -> bool { false }
}
impl<T: InertAtomic> StrictEq for T {
  fn strict_eq(&self, other: &dyn Any) -> bool {
    other.downcast_ref().map_or(false, |other| self.strict_eq(other))
  }
}
impl<T: InertAtomic> Responder for T {
  fn respond(&self, mut request: Request) {
    if request.can_serve::<T>() {
      request.serve(self.clone())
    } else {
      self.respond(request)
    }
  }
}
impl<T: InertAtomic> Atomic for T {
  fn as_any(self: Box<Self>) -> Box<dyn Any> { self }
  fn as_any_ref(&self) -> &dyn Any { self }

  fn run(self: Box<Self>, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn { gas: ctx.gas, inert: true, clause: self.atom_cls() })
  }
}

impl<T: InertAtomic> TryFromExprInst for T {
  fn from_exi(exi: ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    let Expr { clause, location } = exi.expr_val();
    match clause {
      Clause::Atom(a) => match a.0.as_any().downcast() {
        Ok(t) => Ok(*t),
        Err(_) => AssertionError::fail(location, Self::type_str()),
      },
      _ => AssertionError::fail(location, "atom"),
    }
  }
}

impl InertAtomic for bool {
  fn type_str() -> &'static str { "bool" }
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}

impl InertAtomic for usize {
  fn type_str() -> &'static str { "usize" }
  fn strict_eq(&self, other: &Self) -> bool { self == other }
  fn respond(&self, mut request: Request) {
    request.serve(Numeric::Uint(*self))
  }
}

impl InertAtomic for NotNan<f64> {
  fn type_str() -> &'static str { "NotNan<f64>" }
  fn strict_eq(&self, other: &Self) -> bool { self == other }
  fn respond(&self, mut request: Request) {
    request.serve(Numeric::Float(*self))
  }
}
