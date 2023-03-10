
use chumsky::Parser;

use std::fmt::Debug;
use std::hash::Hash;

use super::super::assertion_error::AssertionError;
use crate::parse::float_parser;
use crate::{atomic_impl, atomic_redirect, externfn_impl};
use crate::foreign::ExternError;
use crate::representations::{Primitive, Literal};
use crate::representations::interpreted::Clause;

/// ParseFloat a number
/// 
/// Next state: [ParseFloat0]

#[derive(Clone)]
pub struct ParseFloat1;
externfn_impl!(ParseFloat1, |_: &Self, c: Clause| {Ok(ParseFloat0{c})});

/// Applied to_string function
/// 
/// Prev state: [ParseFloat1]

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct ParseFloat0{ c: Clause }
atomic_redirect!(ParseFloat0, c);
atomic_impl!(ParseFloat0, |Self{ c }: &Self| {
  let literal: &Literal = c.try_into()
    .map_err(|_| AssertionError::ext(c.clone(), "a literal value"))?;
  let number = match literal {
    Literal::Str(s) => {
      let parser = float_parser();
      parser.parse(s.as_str()).map_err(|_| AssertionError{
        value: c.clone(), assertion: "cannot be parsed into a float"
      }.into_extern())?
    }
    Literal::Num(n) => *n,
    Literal::Uint(i) => (*i as u32).into(),
    Literal::Char(char) => char.to_digit(10).ok_or(AssertionError{
      value: c.clone(), assertion: "is not a decimal digit"
    }.into_extern())?.into()
  };
  Ok(Clause::P(Primitive::Literal(Literal::Num(number))))
});