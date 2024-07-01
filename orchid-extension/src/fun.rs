use std::borrow::Cow;

use dyn_clone::{clone_box, DynClone};
use never::Never;
use trait_set::trait_set;

use crate::atom::Atomic;
use crate::atom_owned::{OwnedAtom, OwnedVariant};
use crate::expr::{ExprHandle, GenExpr};
use crate::system::SysCtx;
use crate::conv::{ToExpr, TryFromExpr};

trait_set! {
  trait FunCB = FnOnce(ExprHandle) -> GenExpr + DynClone + Send + Sync + 'static;
}

pub struct Fun(Box<dyn FunCB>);
impl Fun {
  pub fn new<I: TryFromExpr, O: ToExpr>(f: impl FnOnce(I) -> O + Clone + Send + Sync + 'static) -> Self {
    Self(Box::new(|eh| I::try_from_expr(eh).map(f).to_expr()))
  }
}
impl Clone for Fun {
  fn clone(&self) -> Self { Self(clone_box(&*self.0)) }
}
impl Atomic for Fun {
  type Data = ();
  type Req = Never;
  type Variant = OwnedVariant;
}
impl OwnedAtom for Fun {
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
  fn call_ref(&self, arg: ExprHandle) -> GenExpr { self.clone().call(arg) }
  fn call(self, arg: ExprHandle) -> GenExpr { (self.0)(arg) }
  fn handle_req(&self, _ctx: SysCtx, req: Self::Req, _rep: &mut (impl std::io::Write + ?Sized)) {
    match req {}
  }
}
