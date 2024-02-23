//! Helper for generating the interpreter's internal representation

use std::cell::RefCell;
use std::mem;

use substack::Substack;

use super::nort::{AsDerefMut, Clause, Expr};
use super::path_set::PathSet;
use crate::utils::pure_seq::pushed;

enum IntGenData<'a, T: ?Sized> {
  Lambda(&'a T, &'a RefCell<Option<PathSet>>),
  /// Counts left steps within a chain of [Clause::Apply] for collapsing.
  Apply(&'a RefCell<usize>),
  /// Replaces [IntGenData::Apply] when stepping left into non-apply to record
  /// a [None] [super::path_set::Step].
  AppF,
  /// Replaces [IntGenData::Apply] when stepping right to freeze the value.
  AppArg(usize),
}

impl<'a, T: ?Sized> Copy for IntGenData<'a, T> {}
impl<'a, T: ?Sized> Clone for IntGenData<'a, T> {
  fn clone(&self) -> Self { *self }
}

struct ArgCollector(RefCell<Option<PathSet>>);
impl ArgCollector {
  pub fn new() -> Self { Self(RefCell::new(None)) }
  pub fn into_path(self) -> Option<PathSet> { self.0.into_inner() }
}

/// Strategy used to find the lambda corresponding to a given argument in the
/// stack. The function is called on the data associated with the argument, then
/// the callback it returns is called on every lambda ancestor's associated
/// data from closest to outermost ancestor. The first lambda where this
/// callback returns true is considered to own the argument.
pub type LambdaPicker<'a, T, U> = &'a dyn for<'b> Fn(&'b U) -> Box<dyn FnMut(&T) -> bool + 'b>;

/// Bundle of information passed down through recursive fnuctions to instantiate
/// runtime [Expr], [super::nort::ClauseInst] or [Clause].
///
/// The context used by [crate::gen::traits::Gen] to convert templates is which
/// includes this type is constructed with [super::gen_nort::nort_gen].
pub struct NortBuilder<'a, T: ?Sized, U: ?Sized> {
  stack: Substack<'a, IntGenData<'a, T>>,
  lambda_picker: LambdaPicker<'a, T, U>,
}
impl<'a, T: ?Sized, U: ?Sized> NortBuilder<'a, T, U> {
  /// Create a new recursive [super::nort] builder from a location that will be
  pub fn new(lambda_picker: LambdaPicker<'a, T, U>) -> Self {
    Self { stack: Substack::Bottom, lambda_picker }
  }
  /// [Substack::pop] and clone the location
  fn pop<'b>(&'b self, count: usize) -> NortBuilder<'b, T, U>
  where 'a: 'b {
    let mut new = *self;
    new.stack = *self.stack.pop(count);
    new
  }
  /// [Substack::push] and clone the location
  fn push<'b>(&'b self, data: IntGenData<'a, T>) -> NortBuilder<'b, T, U>
  where 'a: 'b {
    let mut new = *self;
    new.stack = self.stack.push(data);
    new
  }
  fn non_app_step<V>(self, f: impl FnOnce(NortBuilder<T, U>) -> V) -> V {
    if let Some(IntGenData::Apply(_)) = self.stack.value() {
      f(self.pop(1).push(IntGenData::AppF))
    } else {
      f(self)
    }
  }

  /// Climb back through the stack and find a lambda associated with this
  /// argument, then record the path taken from the lambda to this argument in
  /// the lambda's mutable cell.
  pub fn arg_logic(self, name: &'a U) {
    let mut lambda_chk = (self.lambda_picker)(name);
    self.non_app_step(|ctx| {
      let res = ctx.stack.iter().try_fold(vec![], |path, item| match item {
        IntGenData::Apply(_) => panic!("This is removed after handling"),
        IntGenData::Lambda(n, rc) => match lambda_chk(n) {
          false => Ok(path),
          true => Err((path, *rc)),
        },
        IntGenData::AppArg(n) => Ok(pushed(path, Some(*n))),
        IntGenData::AppF => Ok(pushed(path, None)),
      });
      let (mut path, slot) = res.expect_err("Argument not wrapped in matching lambda");
      path.reverse();
      match &mut *slot.borrow_mut() {
        slot @ None => *slot = Some(PathSet::end(path)),
        Some(slot) => take_mut::take(slot, |p| p.overlay(PathSet::end(path))),
      }
    })
  }

  /// Push a stackframe corresponding to a lambda expression, build the body,
  /// then record the path set collected by [NortBuilder::arg_logic] calls
  /// within the body.
  pub fn lambda_logic(self, name: &T, body: impl FnOnce(NortBuilder<T, U>) -> Expr) -> Clause {
    let coll = ArgCollector::new();
    let frame = IntGenData::Lambda(name, &coll.0);
    let body = self.non_app_step(|ctx| body(ctx.push(frame)));
    let args = coll.into_path();
    Clause::Lambda { args, body }
  }

  /// Logic for collapsing Apply clauses. Different steps of the logic
  /// communicate via mutable variables on the stack
  pub fn apply_logic(
    self,
    f: impl FnOnce(NortBuilder<T, U>) -> Expr,
    x: impl FnOnce(NortBuilder<T, U>) -> Expr,
  ) -> Clause {
    let mut fun: Expr;
    let arg: Expr;
    if let Some(IntGenData::Apply(rc)) = self.stack.value() {
      // argument side commits backidx
      arg = x(self.pop(1).push(IntGenData::AppArg(*rc.borrow())));
      // function side increments backidx
      *rc.borrow_mut() += 1;
      fun = f(self);
    } else {
      // function side starts from backidx 1
      fun = f(self.push(IntGenData::Apply(&RefCell::new(1))));
      // argument side commits 0
      arg = x(self.push(IntGenData::AppArg(0)));
    };
    let mut cls_lk = fun.as_deref_mut();
    if let Clause::Apply { x, f: _ } = &mut *cls_lk {
      x.push_back(arg);
      mem::drop(cls_lk);
      fun.clause.into_cls()
    } else {
      mem::drop(cls_lk);
      Clause::Apply { f: fun, x: [arg].into() }
    }
  }
}

impl<'a, T: ?Sized, U: ?Sized> Copy for NortBuilder<'a, T, U> {}
impl<'a, T: ?Sized, U: ?Sized> Clone for NortBuilder<'a, T, U> {
  fn clone(&self) -> Self { *self }
}
