use std::any::Any;
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};

use ordered_float::NotNan;

use super::atom::{Atom, Atomic, AtomicResult, AtomicReturn, NotAFunction};
use super::error::{ExternError, ExternResult};
use super::try_from_expr::TryFromExpr;
use crate::foreign::error::AssertionError;
use crate::interpreter::apply::CallData;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::interpreter::run::RunData;
use crate::libs::std::number::Numeric;
use crate::libs::std::string::OrcString;
use crate::utils::ddispatch::{Request, Responder};

/// A proxy trait that implements [Atomic] for blobs of data in Rust code that
/// cannot be processed and always report inert. Since these are expected to be
/// parameters of functions defined with [define_fn] it also automatically
/// implements [TryFromExpr] so that a conversion doesn't have to be
/// provided in argument lists.
pub trait InertPayload: Debug + Clone + Send + 'static {
  /// Typename to be shown in the error when a conversion from [ExprInst] fails
  ///
  /// This will default to `type_name::<Self>()` when it becomes stable
  const TYPE_STR: &'static str;
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

/// An atom that stores a value and rejects all interpreter interactions. It is
/// used to reference foreign data in Orchid.
#[derive(Debug, Clone)]
pub struct Inert<T: InertPayload>(pub T);
impl<T: InertPayload> Inert<T> {
  /// Wrap the argument in a type-erased [Atom] for embedding in Orchid
  /// structures.
  pub fn atom(t: T) -> Atom { Atom::new(Inert(t)) }
}

impl<T: InertPayload> Deref for Inert<T> {
  type Target = T;
  fn deref(&self) -> &Self::Target { &self.0 }
}

impl<T: InertPayload> DerefMut for Inert<T> {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<T: InertPayload> Responder for Inert<T> {
  fn respond(&self, mut request: Request) {
    if request.can_serve::<T>() {
      request.serve(self.0.clone())
    } else {
      self.0.respond(request)
    }
  }
}
impl<T: InertPayload> Atomic for Inert<T> {
  fn as_any(self: Box<Self>) -> Box<dyn Any> { self }
  fn as_any_ref(&self) -> &dyn Any { self }

  fn redirect(&mut self) -> Option<&mut ClauseInst> { None }
  fn run(self: Box<Self>, run: RunData) -> AtomicResult {
    AtomicReturn::inert(*self, run.ctx)
  }
  fn apply_ref(&self, call: CallData) -> ExternResult<Clause> {
    Err(NotAFunction(self.clone().atom_expr(call.location)).rc())
  }
  fn parser_eq(&self, other: &dyn Any) -> bool {
    (other.downcast_ref::<Self>())
      .map_or(false, |other| self.0.strict_eq(&other.0))
  }
}

impl<T: InertPayload> TryFromExpr for Inert<T> {
  fn from_expr(expr: Expr) -> ExternResult<Self> {
    let Expr { clause, location } = expr;
    match clause.try_unwrap() {
      Ok(Clause::Atom(at)) => at.try_downcast::<Self>().map_err(|a| {
        AssertionError::ext(location, T::TYPE_STR, format!("{a:?}"))
      }),
      Err(inst) => match &*inst.cls() {
        Clause::Atom(at) =>
          at.downcast_ref::<Self>().cloned().ok_or_else(|| {
            AssertionError::ext(location, T::TYPE_STR, format!("{inst}"))
          }),
        cls => AssertionError::fail(location, "atom", format!("{cls}")),
      },
      Ok(cls) => AssertionError::fail(location, "atom", format!("{cls}")),
    }
  }
}

impl<T: InertPayload + Display> Display for Inert<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl InertPayload for bool {
  const TYPE_STR: &'static str = "bool";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
  fn respond(&self, mut request: Request) {
    request.serve_with(|| OrcString::from(self.to_string()))
  }
}

impl InertPayload for usize {
  const TYPE_STR: &'static str = "usize";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
  fn respond(&self, mut request: Request) {
    request.serve(Numeric::Uint(*self));
    request.serve_with(|| OrcString::from(self.to_string()))
  }
}

impl InertPayload for NotNan<f64> {
  const TYPE_STR: &'static str = "NotNan<f64>";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
  fn respond(&self, mut request: Request) {
    request.serve(Numeric::Float(*self));
    request.serve_with(|| OrcString::from(self.to_string()))
  }
}
