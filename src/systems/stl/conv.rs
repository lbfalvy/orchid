use chumsky::Parser;
use ordered_float::NotNan;

use super::ArithmeticError;
use crate::foreign::ExternError;
use crate::interner::Interner;
use crate::interpreted::Clause;
use crate::parse::{float_parser, int_parser};
use crate::systems::cast_exprinst::with_lit;
use crate::systems::AssertionError;
use crate::{define_fn, ConstTree, Literal};

define_fn! {
  /// parse a number. Accepts the same syntax Orchid does.
  ToFloat = |x| with_lit(x, |l| match l {
    Literal::Str(s) => float_parser()
      .parse(s.as_str())
      .map_err(|_| AssertionError::ext(
        x.clone(),
        "cannot be parsed into a float"
      )),
    Literal::Num(n) => Ok(*n),
    Literal::Uint(i) => NotNan::new(*i as f64)
      .map_err(|_| ArithmeticError::NaN.into_extern()),
  }).map(|nn| Literal::Num(nn).into())
}

define_fn! {
  /// Parse an unsigned integer. Accepts the same formats Orchid does. If the
  /// input is a number, floors it.
  ToUint = |x| with_lit(x, |l| match l {
    Literal::Str(s) => int_parser()
      .parse(s.as_str())
      .map_err(|_| AssertionError::ext(
        x.clone(),
        "cannot be parsed into an unsigned int",
      )),
    Literal::Num(n) => Ok(n.floor() as u64),
    Literal::Uint(i) => Ok(*i),
  }).map(|u| Literal::Uint(u).into())
}

define_fn! {
  /// Convert a literal to a string using Rust's conversions for floats, chars and
  /// uints respectively
  ToString = |x| with_lit(x, |l| Ok(match l {
    Literal::Uint(i) => Literal::Str(i.to_string().into()),
    Literal::Num(n) => Literal::Str(n.to_string().into()),
    s@Literal::Str(_) => s.clone(),
  })).map(Clause::from)
}

pub fn conv(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("to_float"), ConstTree::xfn(ToFloat)),
    (i.i("to_uint"), ConstTree::xfn(ToUint)),
    (i.i("to_string"), ConstTree::xfn(ToString)),
  ])
}
