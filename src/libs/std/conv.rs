use once_cell::sync::Lazy;
use ordered_float::NotNan;

use super::number::Numeric;
use super::protocol::{gen_resolv, Protocol};
use super::string::OrcString;
use crate::foreign::atom::Atomic;
use crate::foreign::error::{AssertionError, ExternResult};
use crate::foreign::fn_bridge::constructors::xfn_1ary;
use crate::foreign::inert::Inert;
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::{atom_leaf, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort_builder::NortBuilder;
use crate::interpreter::nort::{ClauseInst, Expr};
use crate::parse::numeric::parse_num;

pub static TO_STRING: Lazy<Protocol> =
  Lazy::new(|| Protocol::new("to_string", []));

fn to_numeric(WithLoc(loc, a): WithLoc<ClauseInst>) -> ExternResult<Numeric> {
  if let Some(n) = a.request::<Numeric>() {
    return Ok(n);
  }
  if let Some(s) = a.request::<OrcString>() {
    return parse_num(s.as_str()).map_err(|e| {
      AssertionError::ext(loc, "number syntax", format!("{e:?}"))
    });
  }
  AssertionError::fail(loc, "string or number", format!("{a}"))
}

/// parse a number. Accepts the same syntax Orchid does.
pub fn to_float(a: WithLoc<ClauseInst>) -> ExternResult<Inert<NotNan<f64>>> {
  to_numeric(a).map(|n| Inert(n.as_float()))
}

/// Parse an unsigned integer. Accepts the same formats Orchid does. If the
/// input is a number, floors it.
pub fn to_uint(a: WithLoc<ClauseInst>) -> ExternResult<Inert<usize>> {
  to_numeric(a).map(|n| match n {
    Numeric::Float(f) => Inert(f.floor() as usize),
    Numeric::Uint(i) => Inert(i),
  })
}

/// Convert a literal to a string using Rust's conversions for floats, chars and
/// uints respectively
pub fn to_string(WithLoc(loc, a): WithLoc<Expr>) -> Expr {
  match a.clone().downcast::<Inert<OrcString>>() {
    Ok(str) => str.atom_expr(loc),
    Err(_) => match a.clause.request::<OrcString>() {
      Some(str) => Inert(str).atom_expr(loc),
      None => tpl::a2(gen_resolv("std::to_string"), tpl::Slot, tpl::Slot)
        .template(nort_gen(loc), [a.clone(), a]),
    },
  }
}

pub fn conv_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([
    TO_STRING.as_tree_ent([]),
    ConstTree::tree_ent("conv", [
      ("to_float", atom_leaf(xfn_1ary(to_float))),
      ("to_uint", atom_leaf(xfn_1ary(to_uint))),
      ("to_string", atom_leaf(xfn_1ary(to_string))),
    ]),
  ])])
}
