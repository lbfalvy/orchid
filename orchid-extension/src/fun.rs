use std::borrow::Cow;

use dyn_clone::{clone_box, DynClone};
use never::Never;
use trait_set::trait_set;

use crate::atom::{AtomCard, OwnedAtom};
use crate::expr::{ExprHandle, GenClause};
use crate::system::SysCtx;

trait_set! {
  trait FunCB = FnOnce(ExprHandle) -> GenClause + DynClone + Send + Sync + 'static;
}

pub struct Fun(Box<dyn FunCB>);
impl Fun {
  pub fn new(f: impl FnOnce(ExprHandle) -> GenClause + Clone + Send + Sync + 'static) -> Self {
    Self(Box::new(f))
  }
}
impl Clone for Fun {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}
impl AtomCard for Fun {
  type Data = ();
  type Req = Never;
}
impl OwnedAtom for Fun {
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
  fn call_ref(&self, arg: ExprHandle) -> GenClause { self.clone().call(arg) }
  fn call(self, arg: ExprHandle) -> GenClause { (self.0)(arg) }
  fn handle_req(&self, _ctx: SysCtx, req: Self::Req, _rep: &mut (impl std::io::Write + ?Sized)) {
    match req {}
  }
}
