use std::any::TypeId;

use itertools::Itertools;
use orchid_api::expr::{Clause, Expr};
use orchid_api::location::Location;

use super::traits::{GenClause, Generable};
use crate::expr::RtExpr;
use crate::host::AtomHand;
use crate::intern::{deintern, intern};

fn safely_reinterpret<In: 'static, Out: 'static>(x: In) -> Result<Out, In> {
  if TypeId::of::<In>() != TypeId::of::<Out>() {
    return Err(x);
  }
  let bx = Box::new(x);
  // SAFETY: type sameness asserted above, from_raw and into_raw pair up
  Ok(*unsafe { Box::from_raw(Box::into_raw(bx) as *mut Out) })
}

/// impls of the gen traits for external types

impl GenClause for Expr {
  fn generate<T: super::traits::Generable>(&self, ctx: T::Ctx<'_>, pop: &impl Fn() -> T) -> T {
    match &self.clause {
      Clause::Arg(arg) => T::arg(ctx, deintern(*arg).as_str()),
      Clause::Atom(atom) => T::atom(ctx, AtomHand::from_api(atom.clone())),
      Clause::Call(f, x) => T::apply(ctx, |c| f.generate(c, pop), |c| x.generate(c, pop)),
      Clause::Lambda(arg, b) => T::lambda(ctx, deintern(*arg).as_str(), |ctx| b.generate(ctx, pop)),
      Clause::Seq(n1, n2) => T::seq(ctx, |c| n1.generate(c, pop), |c| n2.generate(c, pop)),
      Clause::Const(int) => T::constant(ctx, deintern(*int).iter().map(|t| t.as_str())),
      Clause::Slot(expr) => {
        let rte = RtExpr::resolve(*expr).expect("expired ticket");
        safely_reinterpret(rte).expect("ticket slots make no sense for anything other than rte")
      },
    }
  }
}

fn to_expr(clause: Clause) -> Expr { Expr { clause, location: Location::None } }

impl Generable for Expr {
  type Ctx<'a> = ();

  fn arg(_ctx: Self::Ctx<'_>, name: &str) -> Self { to_expr(Clause::Arg(intern(name).marker())) }
  fn apply(
    ctx: Self::Ctx<'_>,
    f: impl FnOnce(Self::Ctx<'_>) -> Self,
    x: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    to_expr(Clause::Call(Box::new(f(ctx)), Box::new(x(ctx))))
  }
  fn atom(_ctx: Self::Ctx<'_>, a: crate::host::AtomHand) -> Self { to_expr(Clause::Atom(a.api_ref())) }
  fn constant<'a>(_ctx: Self::Ctx<'_>, name: impl IntoIterator<Item = &'a str>) -> Self {
    to_expr(Clause::Const(intern(&name.into_iter().map(intern).collect_vec()[..]).marker()))
  }
  fn lambda(ctx: Self::Ctx<'_>, name: &str, body: impl FnOnce(Self::Ctx<'_>) -> Self) -> Self {
    to_expr(Clause::Lambda(intern(name).marker(), Box::new(body(ctx))))
  }
  fn seq(
    ctx: Self::Ctx<'_>,
    a: impl FnOnce(Self::Ctx<'_>) -> Self,
    b: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    to_expr(Clause::Seq(Box::new(a(ctx)), Box::new(b(ctx))))
  }
}
