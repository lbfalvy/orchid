use std::fmt::Debug;

use chumsky::Parser;

use super::super::assertion_error::AssertionError;
use crate::external::litconv::with_lit;
use crate::parse::float_parser;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// parse a number. Accepts the same syntax Orchid does
///
/// Next state: [ParseFloat0]
#[derive(Clone)]
pub struct ParseFloat1;
externfn_impl!(ParseFloat1, |_: &Self, x: ExprInst| Ok(ParseFloat0 { x }));

/// Prev state: [ParseFloat1]
#[derive(Debug, Clone)]
pub struct ParseFloat0 {
  x: ExprInst,
}
atomic_redirect!(ParseFloat0, x);
atomic_impl!(ParseFloat0, |Self { x }: &Self, _| {
  let number = with_lit(x, |l| {
    Ok(match l {
      Literal::Str(s) => {
        let parser = float_parser();
        parser.parse(s.as_str()).map_err(|_| {
          AssertionError::ext(x.clone(), "cannot be parsed into a float")
        })?
      },
      Literal::Num(n) => *n,
      Literal::Uint(i) => (*i as u32).into(),
      Literal::Char(char) => char
        .to_digit(10)
        .ok_or(AssertionError::ext(x.clone(), "is not a decimal digit"))?
        .into(),
    })
  })?;
  Ok(number.into())
});
