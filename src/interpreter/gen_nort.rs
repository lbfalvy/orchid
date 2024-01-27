//! Implementations of [Generable] for [super::nort]

use intern_all::i;

use super::nort_builder::NortBuilder;
use crate::foreign::atom::Atom;
use crate::foreign::to_clause::ToClause;
use crate::gen::traits::Generable;
use crate::interpreter::nort::{Clause, ClauseInst, Expr};
use crate::location::CodeLocation;
use crate::name::Sym;

/// Context data for instantiating templated expressions as [super::nort].
/// Instances of this type are created via [nort_gen]
pub type NortGenCtx<'a> = (CodeLocation, NortBuilder<'a, str, str>);

/// Create [NortGenCtx] instances to generate interpreted expressions
pub fn nort_gen<'a>(location: CodeLocation) -> NortGenCtx<'a> {
  (location, NortBuilder::new(&|l| Box::new(move |r| l == r)))
}

impl Generable for Expr {
  type Ctx<'a> = NortGenCtx<'a>;
  fn apply(
    ctx: Self::Ctx<'_>,
    f_cb: impl FnOnce(Self::Ctx<'_>) -> Self,
    x_cb: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    (ctx
      .1
      .apply_logic(|c| f_cb((ctx.0.clone(), c)), |c| x_cb((ctx.0.clone(), c))))
    .to_expr(ctx.0.clone())
  }
  fn arg(ctx: Self::Ctx<'_>, name: &str) -> Self {
    Clause::arg(ctx.clone(), name).to_expr(ctx.0.clone())
  }
  fn atom(ctx: Self::Ctx<'_>, a: Atom) -> Self {
    Clause::atom(ctx.clone(), a).to_expr(ctx.0.clone())
  }
  fn constant<'a>(
    ctx: Self::Ctx<'_>,
    name: impl IntoIterator<Item = &'a str>,
  ) -> Self {
    Clause::constant(ctx.clone(), name).to_expr(ctx.0.clone())
  }
  fn lambda(
    ctx: Self::Ctx<'_>,
    name: &str,
    body: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    (ctx.1.lambda_logic(name, |c| body((ctx.0.clone(), c))))
      .to_expr(ctx.0.clone())
  }
}

impl Generable for ClauseInst {
  type Ctx<'a> = NortGenCtx<'a>;
  fn arg(ctx: Self::Ctx<'_>, name: &str) -> Self {
    Clause::arg(ctx, name).to_inst()
  }
  fn atom(ctx: Self::Ctx<'_>, a: Atom) -> Self {
    Clause::atom(ctx, a).to_inst()
  }
  fn constant<'a>(
    ctx: Self::Ctx<'_>,
    name: impl IntoIterator<Item = &'a str>,
  ) -> Self {
    Clause::constant(ctx, name).to_inst()
  }
  fn lambda(
    ctx: Self::Ctx<'_>,
    name: &str,
    body: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    (ctx
      .1
      .lambda_logic(name, |c| body((ctx.0.clone(), c)).to_expr(ctx.0.clone())))
    .to_clsi(ctx.0.clone())
  }
  fn apply(
    ctx: Self::Ctx<'_>,
    f: impl FnOnce(Self::Ctx<'_>) -> Self,
    x: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    (ctx.1.apply_logic(
      |c| f((ctx.0.clone(), c)).to_expr(ctx.0.clone()),
      |c| x((ctx.0.clone(), c)).to_expr(ctx.0.clone()),
    ))
    .to_clsi(ctx.0.clone())
  }
}

impl Generable for Clause {
  type Ctx<'a> = NortGenCtx<'a>;
  fn atom(_: Self::Ctx<'_>, a: Atom) -> Self { Clause::Atom(a) }
  fn constant<'a>(
    _: Self::Ctx<'_>,
    name: impl IntoIterator<Item = &'a str>,
  ) -> Self {
    let sym = Sym::new(name.into_iter().map(i)).expect("Empty constant");
    Clause::Constant(sym)
  }
  fn apply(
    ctx: Self::Ctx<'_>,
    f: impl FnOnce(Self::Ctx<'_>) -> Self,
    x: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    ctx.1.apply_logic(
      |c| f((ctx.0.clone(), c)).to_expr(ctx.0.clone()),
      |c| x((ctx.0.clone(), c)).to_expr(ctx.0.clone()),
    )
  }
  fn arg(ctx: Self::Ctx<'_>, name: &str) -> Self {
    ctx.1.arg_logic(name);
    Clause::LambdaArg
  }
  fn lambda(
    ctx: Self::Ctx<'_>,
    name: &str,
    body: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self {
    ctx
      .1
      .lambda_logic(name, |c| body((ctx.0.clone(), c)).to_expr(ctx.0.clone()))
  }
}
