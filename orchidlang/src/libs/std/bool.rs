use super::number::Numeric;
use super::string::OrcString;
use crate::foreign::error::{AssertionError, RTResult};
use crate::foreign::inert::Inert;
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tpl;
use crate::gen::traits::{Gen, GenClause};
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::Expr;

const fn left() -> impl GenClause { tpl::L("l", tpl::L("_", tpl::P("l"))) }
const fn right() -> impl GenClause { tpl::L("_", tpl::L("r", tpl::P("r"))) }

/// Takes a boolean and two branches, runs the first if the bool is true, the
/// second if it's false.
// Even though it's a ternary function, IfThenElse is implemented as an unary
// foreign function, as the rest of the logic can be defined in Orchid.
pub fn if_then_else(WithLoc(loc, b): WithLoc<Inert<bool>>) -> Expr {
  let ctx = nort_gen(loc);
  if b.0 { left().template(ctx, []) } else { right().template(ctx, []) }
}

/// Compares the inner values if
///
/// - both are string,
/// - both are bool,
/// - both are either uint or num
pub fn equals(WithLoc(loc, a): WithLoc<Expr>, b: Expr) -> RTResult<Inert<bool>> {
  Ok(Inert(if let Ok(l) = a.clone().downcast::<Inert<OrcString>>() {
    b.downcast::<Inert<OrcString>>().is_ok_and(|r| *l == *r)
  } else if let Ok(l) = a.clone().downcast::<Inert<bool>>() {
    b.downcast::<Inert<bool>>().is_ok_and(|r| *l == *r)
  } else if let Some(l) = a.clause.request::<Numeric>() {
    b.clause.request::<Numeric>().is_some_and(|r| l.as_float() == r.as_float())
  } else {
    AssertionError::fail(loc, "string, bool or numeric", format!("{a}"))?
  }))
}

pub fn bool_lib() -> ConstTree {
  ConstTree::ns("std::bool", [ConstTree::tree([
    xfn_ent("ifthenelse", [if_then_else]),
    xfn_ent("equals", [equals]),
    atom_ent("true", [Inert(true)]),
    atom_ent("false", [Inert(false)]),
  ])])
}
