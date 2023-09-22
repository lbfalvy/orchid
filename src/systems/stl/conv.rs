use chumsky::Parser;
use ordered_float::NotNan;

use super::ArithmeticError;
use crate::foreign::{xfn_1ary, ExternError, XfnResult};
use crate::interner::Interner;
use crate::parse::{float_parser, int_parser};
use crate::systems::AssertionError;
use crate::{ConstTree, Literal, Location};

/// parse a number. Accepts the same syntax Orchid does.
pub fn to_float(l: Literal) -> XfnResult<Literal> {
  match l {
    Literal::Str(s) => float_parser()
      .parse(s.as_str())
      .map(Literal::Num)
      .map_err(|_| AssertionError::ext(Location::Unknown, "float syntax")),
    n @ Literal::Num(_) => Ok(n),
    Literal::Uint(i) => NotNan::new(i as f64)
      .map(Literal::Num)
      .map_err(|_| ArithmeticError::NaN.into_extern()),
  }
}

/// Parse an unsigned integer. Accepts the same formats Orchid does. If the
/// input is a number, floors it.
pub fn to_uint(l: Literal) -> XfnResult<Literal> {
  match l {
    Literal::Str(s) => int_parser()
      .parse(s.as_str())
      .map(Literal::Uint)
      .map_err(|_| AssertionError::ext(Location::Unknown, "int syntax")),
    Literal::Num(n) => Ok(Literal::Uint(n.floor() as u64)),
    i @ Literal::Uint(_) => Ok(i),
  }
}

/// Convert a literal to a string using Rust's conversions for floats, chars and
/// uints respectively
pub fn to_string(l: Literal) -> XfnResult<Literal> {
  Ok(match l {
    Literal::Uint(i) => Literal::Str(i.to_string().into()),
    Literal::Num(n) => Literal::Str(n.to_string().into()),
    s @ Literal::Str(_) => s,
  })
}

pub fn conv(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("to_float"), ConstTree::xfn(xfn_1ary(to_float))),
    (i.i("to_uint"), ConstTree::xfn(xfn_1ary(to_uint))),
    (i.i("to_string"), ConstTree::xfn(xfn_1ary(to_string))),
  ])
}
