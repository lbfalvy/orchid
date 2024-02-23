use ordered_float::NotNan;

use super::number::Numeric;
use super::string::OrcString;
use crate::foreign::error::{AssertionError, RTResult};
use crate::foreign::inert::Inert;
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tpl;
use crate::gen::tree::{leaf, xfn_ent, ConstTree};
use crate::interpreter::nort::ClauseInst;
use crate::parse::numeric::parse_num;

fn to_numeric(WithLoc(loc, a): WithLoc<ClauseInst>) -> RTResult<Numeric> {
  if let Some(n) = a.request::<Numeric>() {
    return Ok(n);
  }
  if let Some(s) = a.request::<OrcString>() {
    return parse_num(s.as_str())
      .map_err(|e| AssertionError::ext(loc, "number syntax", format!("{e:?}")));
  }
  AssertionError::fail(loc, "string or number", format!("{a}"))
}

/// parse a number. Accepts the same syntax Orchid does.
pub fn to_float(a: WithLoc<ClauseInst>) -> RTResult<Inert<NotNan<f64>>> {
  to_numeric(a).map(|n| Inert(n.as_float()))
}

/// Parse an unsigned integer. Accepts the same formats Orchid does. If the
/// input is a number, floors it.
pub fn to_uint(a: WithLoc<ClauseInst>) -> RTResult<Inert<usize>> {
  to_numeric(a).map(|n| match n {
    Numeric::Float(f) => Inert(f.floor() as usize),
    Numeric::Uint(i) => Inert(i),
  })
}

pub fn conv_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([ConstTree::tree_ent("conv", [
    xfn_ent("to_float", [to_float]),
    xfn_ent("to_uint", [to_uint]),
    // conversion logic moved to the string library
    ("to_string", leaf(tpl::C("std::string::convert"))),
  ])])])
}
