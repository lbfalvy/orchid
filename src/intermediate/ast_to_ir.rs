//! Convert the preprocessed AST into IR

use std::collections::VecDeque;
use std::rc::Rc;

use substack::Substack;

use super::ir;
use crate::error::{ProjectError, ProjectResult};
use crate::location::{CodeLocation, SourceRange};
use crate::name::Sym;
use crate::parse::parsed;
use crate::utils::unwrap_or::unwrap_or;

trait IRErrorKind: Clone + Send + Sync + 'static {
  const DESCR: &'static str;
}

#[derive(Clone)]
struct IRError<T: IRErrorKind>(SourceRange, Sym, T);
impl<T: IRErrorKind> ProjectError for IRError<T> {
  const DESCRIPTION: &'static str = T::DESCR;
  fn message(&self) -> String { format!("In {}, {}", self.1, T::DESCR) }
  fn one_position(&self) -> CodeLocation {
    CodeLocation::Source(self.0.clone())
  }
}

#[derive(Clone)]
struct EmptyS;
impl IRErrorKind for EmptyS {
  const DESCR: &'static str =
    "`()` as a clause is meaningless in lambda calculus";
}

#[derive(Clone)]
struct BadGroup;
impl IRErrorKind for BadGroup {
  const DESCR: &'static str = "Only `(...)` may be used after macros. \
  `[...]` and `{...}` left in the code are signs of incomplete macro execution";
}

#[derive(Clone)]
struct InvalidArg;
impl IRErrorKind for InvalidArg {
  const DESCR: &'static str = "Argument names can only be Name nodes";
}

#[derive(Clone)]
struct PhLeak;
impl IRErrorKind for PhLeak {
  const DESCR: &'static str = "Placeholders shouldn't even appear \
    in the code during macro execution, this is likely a compiler bug";
}

/// Try to convert an expression from AST format to typed lambda
pub fn ast_to_ir(expr: parsed::Expr, symbol: Sym) -> ProjectResult<ir::Expr> {
  expr_rec(expr, Context::new(symbol))
}

#[derive(Clone)]
struct Context<'a> {
  names: Substack<'a, Sym>,
  symbol: Sym,
}

impl<'a> Context<'a> {
  #[must_use]
  fn w_name<'b>(&'b self, name: Sym) -> Context<'b>
  where 'a: 'b {
    Context { names: self.names.push(name), symbol: self.symbol.clone() }
  }
}
impl Context<'static> {
  #[must_use]
  fn new(symbol: Sym) -> Self { Self { names: Substack::Bottom, symbol } }
}

/// Process an expression sequence
fn exprv_rec(
  mut v: VecDeque<parsed::Expr>,
  ctx: Context<'_>,
  location: SourceRange,
) -> ProjectResult<ir::Expr> {
  let last = unwrap_or! {v.pop_back(); {
    return Err(IRError(location, ctx.symbol, EmptyS).pack());
  }};
  let v_end = match v.back() {
    None => return expr_rec(last, ctx),
    Some(penultimate) => penultimate.range.range.end,
  };
  let f = exprv_rec(v, ctx.clone(), location.map_range(|r| r.start..v_end))?;
  let x = expr_rec(last, ctx)?;
  let value = ir::Clause::Apply(Rc::new(f), Rc::new(x));
  Ok(ir::Expr { value, location: CodeLocation::Source(location) })
}

/// Process an expression
fn expr_rec(
  parsed::Expr { value, range }: parsed::Expr,
  ctx: Context,
) -> ProjectResult<ir::Expr> {
  match value {
    parsed::Clause::S(parsed::PType::Par, body) => {
      return exprv_rec(body.to_vec().into(), ctx, range);
    },
    parsed::Clause::S(..) =>
      return Err(IRError(range, ctx.symbol, BadGroup).pack()),
    _ => (),
  }
  let value = match value {
    parsed::Clause::Atom(a) => ir::Clause::Atom(a.clone()),
    parsed::Clause::Lambda(arg, b) => {
      let name = match &arg[..] {
        [parsed::Expr { value: parsed::Clause::Name(name), .. }] => name,
        [parsed::Expr { value: parsed::Clause::Placeh { .. }, .. }] =>
          return Err(IRError(range.clone(), ctx.symbol, PhLeak).pack()),
        _ => return Err(IRError(range.clone(), ctx.symbol, InvalidArg).pack()),
      };
      let body_ctx = ctx.w_name(name.clone());
      let body = exprv_rec(b.to_vec().into(), body_ctx, range.clone())?;
      ir::Clause::Lambda(Rc::new(body))
    },
    parsed::Clause::Name(name) => {
      let lvl_opt = (ctx.names.iter())
        .enumerate()
        .find(|(_, n)| **n == name)
        .map(|(lvl, _)| lvl);
      match lvl_opt {
        Some(lvl) => ir::Clause::LambdaArg(lvl),
        None => ir::Clause::Constant(name.clone()),
      }
    },
    parsed::Clause::S(parsed::PType::Par, entries) =>
      exprv_rec(entries.to_vec().into(), ctx, range.clone())?.value,
    parsed::Clause::S(..) =>
      return Err(IRError(range, ctx.symbol, BadGroup).pack()),
    parsed::Clause::Placeh { .. } =>
      return Err(IRError(range, ctx.symbol, PhLeak).pack()),
  };
  Ok(ir::Expr::new(value, range.clone()))
}
