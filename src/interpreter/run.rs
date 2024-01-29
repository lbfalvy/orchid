use std::mem;

use super::context::{Halt, RunContext};
use super::error::RunError;
use super::nort::{Clause, Expr};
use crate::foreign::atom::{AtomicReturn, RunData};
use crate::foreign::error::ExternError;
use crate::interpreter::apply::{apply_as_atom, substitute};
use crate::interpreter::error::{strace, MissingSymbol, StackOverflow};
use crate::utils::pure_seq::pushed;

/// Interpreter state when processing was interrupted
#[derive(Debug, Clone)]
pub struct Interrupted {
  /// Cached soft stack to save the interpreter having to rebuild it from the
  /// bottom.
  pub stack: Vec<Expr>,
}
impl Interrupted {
  /// Continue processing where it was interrupted
  pub fn resume(self, ctx: RunContext) -> Result<Halt, RunError> { run_stack(self.stack, ctx) }
}

/// Normalize an expression using beta reduction with memoization
pub fn run(expr: Expr, ctx: RunContext) -> Result<Halt, RunError> {
  let mut v = Vec::with_capacity(1000);
  v.push(expr);
  run_stack(v, ctx)
}

fn run_stack(mut stack: Vec<Expr>, mut ctx: RunContext) -> Result<Halt, RunError> {
  let mut expr = stack.pop().expect("Empty stack");
  let mut popped = false;
  loop {
    // print!("Now running {expr}");
    // let trace = strace(&stack);
    // if trace.is_empty() {
    //   println!("\n")
    // } else {
    //   println!("\n{trace}\n")
    // };
    if ctx.no_gas() {
      return Err(RunError::Interrupted(Interrupted { stack: pushed(stack, expr) }));
    }
    ctx.use_gas(1);
    enum Res {
      Inert,
      Cont,
      Push(Expr),
    }
    let (next_clsi, res) = expr.clause.try_normalize(|cls| match cls {
      Clause::Identity(_) => panic!("Passed by try_normalize"),
      Clause::LambdaArg => panic!("Unbound argument"),
      Clause::Lambda { .. } => Ok((cls, Res::Inert)),
      Clause::Bottom(b) => Err(b),
      Clause::Constant(n) => match ctx.symbols.get(&n) {
        Some(expr) => Ok((Clause::Identity(expr.clsi()), Res::Cont)),
        None => Err(RunError::Extern(MissingSymbol { sym: n.clone(), loc: expr.location() }.rc())),
      },
      Clause::Atom(mut a) => {
        if !popped {
          if let Some(delegate) = a.0.redirect() {
            let next = delegate.clone();
            return Ok((Clause::Atom(a), Res::Push(next)));
          }
        }
        let rd = RunData { ctx: ctx.clone(), location: expr.location() };
        match a.run(rd)? {
          AtomicReturn::Inert(c) => Ok((c, Res::Inert)),
          AtomicReturn::Change(gas, c) => {
            ctx.use_gas(gas);
            Ok((c, Res::Cont))
          },
        }
      },
      Clause::Apply { f, mut x } => {
        if x.is_empty() {
          return Ok((Clause::Identity(f.clsi()), Res::Cont));
        }
        match &*f.cls() {
          Clause::Identity(f2) =>
            return Ok((Clause::Apply { f: f2.clone().to_expr(f.location()), x }, Res::Cont)),
          Clause::Apply { f, x: x2 } => {
            for item in x2.iter().rev() {
              x.push_front(item.clone())
            }
            return Ok((Clause::Apply { f: f.clone(), x }, Res::Cont));
          },
          _ => (),
        }
        if !popped {
          return Ok((Clause::Apply { f: f.clone(), x }, Res::Push(f)));
        }
        let f_cls = f.cls();
        let arg = x.pop_front().expect("checked above");
        let loc = f.location();
        let f = match &*f_cls {
          Clause::Atom(_) => {
            mem::drop(f_cls);
            apply_as_atom(f, arg, ctx.clone())?
          },
          Clause::Lambda { args, body } => match args {
            None => body.clsi().into_cls(),
            Some(args) => substitute(args, arg.clsi(), &body.cls(), &mut || ctx.use_gas(1)),
          },
          c => panic!("Run should never settle on {c}"),
        };
        Ok((Clause::Apply { f: f.to_expr(loc), x }, Res::Cont))
      },
    })?;
    expr.clause = next_clsi;
    popped = matches!(res, Res::Inert);
    match res {
      Res::Cont => continue,
      Res::Inert => match stack.pop() {
        None => return Ok(Halt { state: expr, gas: ctx.gas, inert: true }),
        Some(prev) => expr = prev,
      },
      Res::Push(next) => {
        if stack.len() == ctx.stack_size {
          stack.extend([expr, next]);
          return Err(RunError::Extern(StackOverflow { stack }.rc()));
        }
        stack.push(expr);
        expr = next;
      },
    }
  }
}
