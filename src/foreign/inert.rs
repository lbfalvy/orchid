use std::any::Any;
use std::fmt::Debug;
use std::rc::Rc;

use super::{AtomicResult, AtomicReturn, ExternError};
#[allow(unused)] // for doc
use crate::define_fn;
use crate::foreign::Atomic;
use crate::interpreted::{ExprInst, TryFromExprInst};
use crate::interpreter::Context;
use crate::systems::cast_exprinst::with_atomic;
use crate::utils::ddispatch::{Request, Responder};

/// A proxy trait that implements [Atomic] for blobs of data in Rust code that
/// cannot be processed and always report inert. Since these are expected to be
/// parameters of functions defined with [define_fn] it also automatically
/// implements [TryFromExprInst] so that a conversion doesn't have to be
/// provided in argument lists.
pub trait InertAtomic: Debug + Clone + 'static {
  /// Typename to be shown in the error when a conversion from [ExprInst] fails
  fn type_str() -> &'static str;
  /// Proxies to [Responder] so that you don't have to implmeent it manually if
  /// you need it, but behaves exactly as the default implementation.
  #[allow(unused_mut, unused_variables)] // definition should show likely usage
  fn respond(&self, mut request: Request) {}
}
impl<T: InertAtomic> Responder for T {
  fn respond(&self, request: Request) { self.respond(request) }
}
impl<T: InertAtomic> Atomic for T {
  fn as_any(&self) -> &dyn Any { self }

  fn run(&self, ctx: Context) -> AtomicResult {
    Ok(AtomicReturn {
      clause: self.clone().atom_cls(),
      gas: ctx.gas,
      inert: true,
    })
  }
}

impl<T: InertAtomic> TryFromExprInst for T {
  fn from_exi(exi: &ExprInst) -> Result<Self, Rc<dyn ExternError>> {
    with_atomic(exi, Self::type_str(), |a: &T| Ok(a.clone()))
  }
}
