use std::fmt::Debug;

use chumsky::Parser;

use super::super::assertion_error::AssertionError;
use super::super::litconv::with_lit;
use crate::parse::int_parser;
use crate::representations::interpreted::ExprInst;
use crate::representations::Literal;
use crate::{atomic_impl, atomic_redirect, externfn_impl};

/// Parse an unsigned integer. Accepts the same formats Orchid does. If the
/// input is a number, floors it.
///
/// Next state: [ParseUint0]
#[derive(Clone)]
pub struct ParseUint1;
externfn_impl!(ParseUint1, |_: &Self, x: ExprInst| Ok(ParseUint0 { x }));

/// Prev state: [ParseUint1]
#[derive(Debug, Clone)]
pub struct ParseUint0 {
  x: ExprInst,
}
atomic_redirect!(ParseUint0, x);
atomic_impl!(ParseUint0, |Self { x }: &Self, _| {
  let uint = with_lit(x, |l| {
    Ok(match l {
      Literal::Str(s) => {
        let parser = int_parser();
        parser.parse(s.as_str()).map_err(|_| {
          AssertionError::ext(
            x.clone(),
            "cannot be parsed into an unsigned int",
          )
        })?
      },
      Literal::Num(n) => n.floor() as u64,
      Literal::Uint(i) => *i,
      Literal::Char(char) => char
        .to_digit(10)
        .ok_or(AssertionError::ext(x.clone(), "is not a decimal digit"))?
        .into(),
    })
  })?;
  Ok(uint.into())
});
