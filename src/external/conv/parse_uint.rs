
use chumsky::Parser;

use std::fmt::Debug;
use std::hash::Hash;

use super::super::assertion_error::AssertionError;
use crate::parse::int_parser;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::foreign::ExternError;
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

/// Parse a number
/// 
/// Next state: [ParseUint0]

#[derive(Clone)]
pub struct ParseUint1;
externfn_impl!(ParseUint1, |_: &Self, c: Clause| {Ok(ParseUint0{c})});

/// Applied ParseUint function
/// 
/// Prev state: [ParseUint1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct ParseUint0{ c: Clause }
atomic_redirect!(ParseUint0, c);
atomic_impl!(ParseUint0, |Self{ c }: &Self| {
  let literal: &Literal = c.try_into()
    .map_err(|_| AssertionError::ext(c.clone(), "a literal value"))?;
  let uint = match literal {
    Literal::Str(s) => {
      let parser = int_parser();
      parser.parse(s.as_str()).map_err(|_| AssertionError{
        value: c.clone(), assertion: "cannot be parsed into an unsigned int"
      }.into_extern())?
    }
    Literal::Num(n) => n.floor() as u64,
    Literal::Uint(i) => *i,
    Literal::Char(char) => char.to_digit(10).ok_or(AssertionError{
      value: c.clone(), assertion: "is not a decimal digit"
    }.into_extern())? as u64
  };
  Ok(Clause::P(Primitive::Literal(Literal::Uint(uint))))
});