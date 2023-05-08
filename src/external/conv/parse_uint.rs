
use chumsky::Parser;

use std::fmt::Debug;

use crate::external::{litconv::with_lit, assertion_error::AssertionError};
use crate::representations::{interpreted::ExprInst, Literal};
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::parse::int_parser;

/// Parse a number
/// 
/// Next state: [ParseUint0]

#[derive(Clone)]
pub struct ParseUint1;
externfn_impl!(ParseUint1, |_: &Self, x: ExprInst| Ok(ParseUint0{x}));

/// Applied ParseUint function
/// 
/// Prev state: [ParseUint1]

#[derive(Debug, Clone)]
pub struct ParseUint0{ x: ExprInst }
atomic_redirect!(ParseUint0, x);
atomic_impl!(ParseUint0, |Self{ x }: &Self| {
  let uint = with_lit(x, |l| Ok(match l {
    Literal::Str(s) => {
      let parser = int_parser();
      parser.parse(s.as_str())
        .map_err(|_| AssertionError::ext(x.clone(), "cannot be parsed into an unsigned int"))?
    }
    Literal::Num(n) => n.floor() as u64,
    Literal::Uint(i) => *i,
    Literal::Char(char) => char.to_digit(10)
      .ok_or(AssertionError::ext(x.clone(), "is not a decimal digit"))?
      .into()
  }))?;
  Ok(uint.into())
});