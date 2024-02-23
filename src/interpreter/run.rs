//! Executes Orchid code

use std::ops::{Deref, DerefMut};
use std::sync::{MutexGuard, TryLockError};
use std::{fmt, mem};

use bound::Bound;
use itertools::Itertools;

use super::context::{Halt, RunEnv, RunParams};
use super::error::RunError;
use super::nort::{Clause, Expr};
use crate::foreign::atom::{AtomicReturn, RunData};
use crate::foreign::error::{RTError, RTErrorObj};
use crate::interpreter::apply::{apply_as_atom, substitute};
use crate::location::CodeLocation;
use crate::utils::take_with_output::take_with_output;

#[derive(Debug)]
struct Stackframe {
  expr: Expr,
  cls: Bound<MutexGuard<'static, Clause>, Expr>,
}
impl Stackframe {
  pub fn new(expr: Expr) -> Option<Self> {
    match Bound::try_new(expr.clone(), |e| e.clause.0.try_lock()) {
      Ok(cls) => Some(Stackframe { cls, expr }),
      Err(bound_e) if matches!(bound_e.wrapped(), TryLockError::WouldBlock) => None,
      Err(bound_e) => panic!("{:?}", bound_e.wrapped()),
    }
  }
  pub fn wait_new(expr: Expr) -> Self {
    Self { cls: Bound::new(expr.clone(), |e| e.clause.0.lock().unwrap()), expr }
  }
  pub fn record_cycle(&mut self) -> RTErrorObj {
    let err = CyclicalExpression(self.expr.clone()).pack();
    *self.cls = Clause::Bottom(err.clone());
    err
  }
}
impl Deref for Stackframe {
  type Target = Clause;
  fn deref(&self) -> &Self::Target { &self.cls }
}
impl DerefMut for Stackframe {
  fn deref_mut(&mut self) -> &mut Self::Target { &mut self.cls }
}
impl fmt::Display for Stackframe {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}\n    at {}", *self.cls, self.expr.location)
  }
}

/// Interpreter state when processing was interrupted
pub struct State<'a> {
  stack: Vec<Stackframe>,
  popped: Option<Expr>,
  env: &'a RunEnv<'a>,
}
impl<'a> State<'a> {
  /// Create a new trivial state with a specified stack size and a single
  /// element on the stack
  fn new(base: Expr, env: &'a RunEnv<'a>) -> Self {
    let stack = vec![Stackframe::new(base).expect("Initial state should not be locked")];
    State { stack, popped: None, env }
  }

  /// Try to push an expression on the stack, raise appropriate errors if the
  /// expression is already on the stack (and thus references itself), or if the
  /// stack now exceeds the pre-defined height
  fn push_expr(&'_ mut self, expr: Expr, params: &RunParams) -> Result<(), RunError<'a>> {
    let sf = match Stackframe::new(expr.clone()) {
      Some(sf) => sf,
      None => match self.stack.iter_mut().rev().find(|sf| sf.expr.clause.is_same(&expr.clause)) {
        None => Stackframe::wait_new(expr),
        Some(sf) => return Err(RunError::Extern(sf.record_cycle())),
      },
    };
    self.stack.push(sf);
    if params.stack < self.stack.len() {
      let so = StackOverflow(self.stack.iter().map(|sf| sf.expr.clone()).collect());
      return Err(RunError::Extern(so.pack()));
    }
    Ok(())
  }

  /// Process this state until it either completes, runs out of gas as specified
  /// in the context, or produces an error.
  pub fn run(mut self, params: &mut RunParams) -> Result<Halt, RunError<'a>> {
    loop {
      if params.no_gas() {
        return Err(RunError::Interrupted(self));
      }
      params.use_gas(1);
      let top = self.stack.last_mut().expect("Stack never empty");
      let location = top.expr.location();
      let op = take_with_output(&mut *top.cls, |c| {
        match step(c, self.popped, location, self.env, params) {
          Err(e) => (Clause::Bottom(e.clone()), Err(RunError::Extern(e))),
          Ok((cls, cmd)) => (cls, Ok(cmd)),
        }
      })?;
      self.popped = None;
      match op {
        StackOp::Nop => continue,
        StackOp::Push(ex) => self.push_expr(ex, params)?,
        StackOp::Swap(ex) => {
          self.stack.pop().expect("Stack never empty");
          self.push_expr(ex, params)?
        },
        StackOp::Pop => {
          let ret = self.stack.pop().expect("last_mut called above");
          if self.stack.is_empty() {
            if let Some(alt) = self.env.dispatch(&ret.cls, ret.expr.location()) {
              self.push_expr(alt, params)?;
              params.use_gas(1);
              continue;
            }
            return Ok(ret.expr);
          } else {
            self.popped = Some(ret.expr);
          }
        },
      }
    }
  }
}
impl<'a> fmt::Display for State<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.stack.iter().rev().join("\n"))
  }
}
impl<'a> fmt::Debug for State<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "State({self})") }
}

/// Process an expression with specific resource limits
pub fn run<'a>(
  base: Expr,
  env: &'a RunEnv<'a>,
  params: &mut RunParams,
) -> Result<Halt, RunError<'a>> {
  State::new(base, env).run(params)
}

enum StackOp {
  Pop,
  Nop,
  Swap(Expr),
  Push(Expr),
}

fn step(
  top: Clause,
  popped: Option<Expr>,
  location: CodeLocation,
  env: &RunEnv,
  params: &mut RunParams,
) -> Result<(Clause, StackOp), RTErrorObj> {
  match top {
    Clause::Bottom(err) => Err(err),
    Clause::LambdaArg => Ok((Clause::Bottom(UnboundArg(location).pack()), StackOp::Nop)),
    l @ Clause::Lambda { .. } => Ok((l, StackOp::Pop)),
    Clause::Identity(other) =>
      Ok((Clause::Identity(other.clone()), StackOp::Swap(other.into_expr(location)))),
    Clause::Constant(name) => {
      let expr = env.load(name, location)?;
      Ok((Clause::Identity(expr.clsi()), StackOp::Swap(expr.clone())))
    },
    Clause::Atom(mut at) => {
      if let Some(delegate) = at.0.redirect() {
        match popped {
          Some(popped) => *delegate = popped,
          None => {
            let tmp = delegate.clone();
            return Ok((Clause::Atom(at), StackOp::Push(tmp)));
          },
        }
      }
      match at.run(RunData { params, env, location })? {
        AtomicReturn::Inert(at) => Ok((Clause::Atom(at), StackOp::Pop)),
        AtomicReturn::Change(gas, c) => {
          params.use_gas(gas);
          Ok((c, StackOp::Nop))
        },
      }
    },
    Clause::Apply { mut f, mut x } => {
      if x.is_empty() {
        return Ok((Clause::Identity(f.clsi()), StackOp::Swap(f)));
      }
      f = match popped {
        None => return Ok((Clause::Apply { f: f.clone(), x }, StackOp::Push(f))),
        Some(ex) => ex,
      };
      let val = x.pop_front().expect("Empty args handled above");
      let f_mut = f.clause.cls_mut();
      let mut cls = match &*f_mut {
        Clause::Lambda { args, body } => match args {
          None => Clause::Identity(body.clsi()),
          Some(args) => substitute(args, val.clsi(), &body.cls_mut(), &mut || params.use_gas(1)),
        },
        Clause::Atom(_) => {
          mem::drop(f_mut);
          apply_as_atom(f, val, env, params)?
        },
        c => unreachable!("Run should never settle on {c}"),
      };
      if !x.is_empty() {
        cls = Clause::Apply { f: cls.into_expr(location), x };
      }
      Ok((cls, StackOp::Nop))
    },
  }
}

#[derive(Clone)]
pub(crate) struct StackOverflow(Vec<Expr>);
impl RTError for StackOverflow {}
impl fmt::Display for StackOverflow {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(f, "Stack depth exceeded {}:", self.0.len() - 1)?; // 1 for failed call, 1 for current
    for item in self.0.iter().rev() {
      match Stackframe::new(item.clone()) {
        Some(sf) => writeln!(f, "{sf}")?,
        None => writeln!(f, "Locked frame at {}", item.location)?,
      }
    }
    Ok(())
  }
}

#[derive(Clone)]
pub(crate) struct UnboundArg(CodeLocation);
impl RTError for UnboundArg {}
impl fmt::Display for UnboundArg {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Unbound argument found at {}. This is likely a codegen error", self.0)
  }
}

#[derive(Clone)]
pub(crate) struct CyclicalExpression(Expr);
impl RTError for CyclicalExpression {}
impl fmt::Display for CyclicalExpression {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "The expression {} contains itself", self.0)
  }
}
