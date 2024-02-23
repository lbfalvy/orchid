//! Abstractions used to generate Orchid expressions

use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;

use crate::foreign::atom::Atom;

/// Representations of the Orchid expression tree that can describe basic
/// language elements.
pub trait Generable: Sized {
  /// Context information defined by parents. Generators just forward this.
  type Ctx<'a>: Sized;
  /// Wrap external data.
  fn atom(ctx: Self::Ctx<'_>, a: Atom) -> Self;
  /// Generate a reference to a constant
  fn constant<'a>(ctx: Self::Ctx<'_>, name: impl IntoIterator<Item = &'a str>) -> Self;
  /// Generate a function call given the function and its argument
  fn apply(
    ctx: Self::Ctx<'_>,
    f: impl FnOnce(Self::Ctx<'_>) -> Self,
    x: impl FnOnce(Self::Ctx<'_>) -> Self,
  ) -> Self;
  /// Generate a function. The argument name is only valid within the same
  /// [Generable].
  fn lambda(ctx: Self::Ctx<'_>, name: &str, body: impl FnOnce(Self::Ctx<'_>) -> Self) -> Self;
  /// Generate a reference to a function argument. The argument name is only
  /// valid within the same [Generable].
  fn arg(ctx: Self::Ctx<'_>, name: &str) -> Self;
}

/// Expression templates which can be instantiated in multiple representations
/// of Orchid. Expressions can be built from the elements defined in
/// [super::tpl].
///
/// Do not depend on this trait, use [Gen] instead. Conversely, implement this
/// instead of [Gen].
pub trait GenClause: fmt::Debug + Sized {
  /// Enact the template at runtime to build a given type.
  /// `pop` pops from the runtime template parameter list passed to the
  /// generator.
  ///
  /// Do not call this, it's the backing operation of [Gen#template]
  fn generate<T: Generable>(&self, ctx: T::Ctx<'_>, pop: &impl Fn() -> T) -> T;
}

/// Expression generators
///
/// Do not implement this trait, it's the frontend for [GenClause]. Conversely,
/// do not consume [GenClause].
pub trait Gen<T: Generable, U>: fmt::Debug {
  /// Create an instance of this template with some parameters
  fn template(&self, ctx: T::Ctx<'_>, params: U) -> T;
}

impl<T: Generable, I: IntoIterator<Item = T>, G: GenClause> Gen<T, I> for G {
  fn template(&self, ctx: T::Ctx<'_>, params: I) -> T {
    let values = RefCell::new(params.into_iter().collect::<VecDeque<_>>());
    let t =
      self.generate(ctx, &|| values.borrow_mut().pop_front().expect("Not enough values provided"));
    let leftover = values.borrow().len();
    assert_eq!(
      leftover,
      0,
      "Too many values provided ({leftover} left) {}",
      Backtrace::force_capture()
    );
    t
  }
}
