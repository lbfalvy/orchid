use chumsky::Parser;
use ordered_float::NotNan;

use super::ArithmeticError;
use crate::foreign::ExternError;
use crate::interner::Interner;
use crate::interpreted::Clause;
use crate::parse::{float_parser, int_parser};
use crate::systems::cast_exprinst::get_literal;
use crate::systems::AssertionError;
use crate::{define_fn, ConstTree, Literal};

define_fn! {
  /// parse a number. Accepts the same syntax Orchid does.
  ToFloat = |x| match get_literal(x)? {
    (Literal::Str(s), loc) => float_parser()
      .parse(s.as_str())
      .map_err(|_| AssertionError::ext(loc, "float syntax")),
    (Literal::Num(n), _) => Ok(n),
    (Literal::Uint(i), _) => NotNan::new(i as f64)
      .map_err(|_| ArithmeticError::NaN.into_extern()),
  }.map(|nn| Literal::Num(nn).into());

  /// Parse an unsigned integer. Accepts the same formats Orchid does. If the
  /// input is a number, floors it.
  ToUint = |x| match get_literal(x)? {
    (Literal::Str(s), loc) => int_parser()
      .parse(s.as_str())
      .map_err(|_| AssertionError::ext(loc, "int syntax")),
    (Literal::Num(n), _) => Ok(n.floor() as u64),
    (Literal::Uint(i), _) => Ok(i),
  }.map(|u| Literal::Uint(u).into());

  /// Convert a literal to a string using Rust's conversions for floats, chars and
  /// uints respectively
  ToString = |x| Ok(match get_literal(x)?.0 {
    Literal::Uint(i) => Clause::from(Literal::Str(i.to_string().into())),
    Literal::Num(n) => Clause::from(Literal::Str(n.to_string().into())),
    s@Literal::Str(_) => Clause::from(s),
  })
}

pub fn conv(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("to_float"), ConstTree::xfn(ToFloat)),
    (i.i("to_uint"), ConstTree::xfn(ToUint)),
    (i.i("to_string"), ConstTree::xfn(ToString)),
  ])
}
