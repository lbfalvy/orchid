//! Various elemental components to build expression trees that all implement
//! [GenClause].

use orchid_api::atom::Atom;

use super::traits::{GenClause, Generable};

/// A trivial atom
#[derive(Clone, Debug)]
pub struct SysAtom(pub Atom);
impl GenClause for SysAtom {
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, _: &impl Fn() -> T) -> T {
    T::atom(ctx, self.0.clone())
  }
}

/// Const, Reference a constant from the execution environment. Unlike Orchid
/// syntax, this doesn't include lambda arguments. For that, use [P]
#[derive(Debug, Clone)]
pub struct C(pub &'static str);
impl GenClause for C {
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, _: &impl Fn() -> T) -> T {
    T::constant(ctx, self.0.split("::"))
  }
}

/// Apply a function to a value provided by [L]
#[derive(Debug, Clone)]
pub struct A<F: GenClause, X: GenClause>(pub F, pub X);
impl<F: GenClause, X: GenClause> GenClause for A<F, X> {
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, p: &impl Fn() -> T) -> T {
    T::apply(ctx, |gen| self.0.generate(gen, p), |gen| self.1.generate(gen, p))
  }
}

/// Apply a function to two arguments
pub fn a2(f: impl GenClause, x: impl GenClause, y: impl GenClause) -> impl GenClause {
  A(A(f, x), y)
}

/// Lambda expression. The argument can be referenced with [P]
#[derive(Debug, Clone)]
pub struct L<B: GenClause>(pub &'static str, pub B);
impl<B: GenClause> GenClause for L<B> {
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, p: &impl Fn() -> T) -> T {
    T::lambda(ctx, self.0, |gen| self.1.generate(gen, p))
  }
}

/// Parameter to a lambda expression
#[derive(Debug, Clone)]
pub struct P(pub &'static str);
impl GenClause for P {
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, _: &impl Fn() -> T) -> T { T::arg(ctx, self.0) }
}

/// Slot for an Orchid value to be specified during execution
#[derive(Debug, Clone)]
pub struct Slot;
impl GenClause for Slot {
  fn generate<T: Generable>(&self, _: T::Ctx<'_>, pop: &impl Fn() -> T) -> T { pop() }
}
