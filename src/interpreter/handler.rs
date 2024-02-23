//! Impure functions that can be triggered by Orchid code when a command
//! evaluates to an atom representing a command

use std::any::{Any, TypeId};
use std::cell::RefCell;

use hashbrown::HashMap;
use trait_set::trait_set;

use super::nort::Expr;
use crate::foreign::atom::Atomic;
use crate::foreign::error::RTResult;
use crate::foreign::to_clause::ToClause;
use crate::location::CodeLocation;

trait_set! {
  trait Handler = for<'a> Fn(&'a dyn Any, CodeLocation) -> Expr;
}

enum HTEntry<'a> {
  Handler(Box<dyn Handler + 'a>),
  Forward(&'a (dyn Handler + 'a)),
}
impl<'a> AsRef<dyn Handler + 'a> for HTEntry<'a> {
  fn as_ref(&self) -> &(dyn Handler + 'a) {
    match self {
      HTEntry::Handler(h) => &**h,
      HTEntry::Forward(h) => *h,
    }
  }
}

/// A table of impure command handlers exposed to Orchid
#[derive(Default)]
pub struct HandlerTable<'a> {
  handlers: HashMap<TypeId, HTEntry<'a>>,
}
impl<'a> HandlerTable<'a> {
  /// Create a new [HandlerTable]
  #[must_use]
  pub fn new() -> Self { Self { handlers: HashMap::new() } }

  /// Add a handler function to interpret a command and select the continuation.
  /// See [HandlerTable#with] for a declarative option.
  pub fn register<T: 'static, R: ToClause>(&mut self, f: impl for<'b> FnMut(&'b T) -> R + 'a) {
    let cell = RefCell::new(f);
    let cb = move |a: &dyn Any, loc: CodeLocation| {
      cell.borrow_mut()(a.downcast_ref().expect("found by TypeId")).to_expr(loc)
    };
    let prev = self.handlers.insert(TypeId::of::<T>(), HTEntry::Handler(Box::new(cb)));
    assert!(prev.is_none(), "A handler for this type is already registered");
  }

  /// Add a handler function to interpret a command and select the continuation.
  /// See [HandlerTable#register] for a procedural option.
  pub fn with<T: 'static>(mut self, f: impl FnMut(&T) -> RTResult<Expr> + 'a) -> Self {
    self.register(f);
    self
  }

  /// Find and execute the corresponding handler for this type
  pub fn dispatch(&self, arg: &dyn Atomic, loc: CodeLocation) -> Option<Expr> {
    (self.handlers.get(&arg.as_any_ref().type_id())).map(|ent| ent.as_ref()(arg.as_any_ref(), loc))
  }

  /// Combine two non-overlapping handler sets
  #[must_use]
  pub fn combine(mut self, other: Self) -> Self {
    for (key, value) in other.handlers {
      let prev = self.handlers.insert(key, value);
      assert!(prev.is_none(), "Duplicate handlers")
    }
    self
  }

  /// Add entries that forward requests to a borrowed non-overlapping handler
  /// set
  pub fn link<'b: 'a>(mut self, other: &'b HandlerTable<'b>) -> Self {
    for (key, value) in other.handlers.iter() {
      let prev = self.handlers.insert(*key, HTEntry::Forward(value.as_ref()));
      assert!(prev.is_none(), "Duplicate handlers")
    }
    self
  }
}

#[cfg(test)]
#[allow(unconditional_recursion)]
#[allow(clippy::ptr_arg)]
mod test {
  use std::marker::PhantomData;

  use super::HandlerTable;

  /// Ensure that the method I use to verify covariance actually passes with
  /// covariant and fails with invariant
  ///
  /// The failing case:
  /// ```
  /// struct Cov2<'a>(PhantomData<&'a mut &'a ()>);
  /// fn fail<'a>(_c: &Cov2<'a>, _s: &'a String) { fail(_c, &String::new()) }
  /// ```
  #[allow(unused)]
  fn covariant_control() {
    struct Cov<'a>(PhantomData<&'a ()>);
    fn pass<'a>(_c: &Cov<'a>, _s: &'a String) { pass(_c, &String::new()) }
  }

  /// The &mut ensures that 'a in the two functions must be disjoint, and that
  /// ht must outlive both. For this to compile, Rust has to cast ht to the
  /// shorter lifetimes, ensuring covariance
  #[allow(unused)]
  fn assert_covariant() {
    fn pass<'a>(_ht: HandlerTable<'a>, _s: &'a String) { pass(_ht, &String::new()) }
  }
}
