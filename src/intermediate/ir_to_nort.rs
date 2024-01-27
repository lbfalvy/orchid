//! Convert IR to the interpreter's NORT representation

use super::ir;
use crate::interpreter::nort;
use crate::interpreter::nort_builder::NortBuilder;

fn expr(expr: &ir::Expr, ctx: NortBuilder<(), usize>) -> nort::Expr {
  clause(&expr.value, ctx).to_expr(expr.location.clone())
}

fn clause(cls: &ir::Clause, ctx: NortBuilder<(), usize>) -> nort::Clause {
  match cls {
    ir::Clause::Constant(name) => nort::Clause::Constant(name.clone()),
    ir::Clause::Atom(a) => nort::Clause::Atom(a.run()),
    ir::Clause::LambdaArg(n) => {
      ctx.arg_logic(n);
      nort::Clause::LambdaArg
    },
    ir::Clause::Apply(f, x) => ctx.apply_logic(|c| expr(f, c), |c| expr(x, c)),
    ir::Clause::Lambda(body) => ctx.lambda_logic(&(), |c| expr(body, c)),
  }
}

pub fn ir_to_nort(expr: &ir::Expr) -> nort::Expr {
  let c = NortBuilder::new(&|count| {
    let mut count: usize = *count;
    Box::new(move |()| count.checked_sub(1).map(|v| count = v).is_none())
  });
  nort::ClauseInst::new(clause(&expr.value, c)).to_expr(expr.location.clone())
}
