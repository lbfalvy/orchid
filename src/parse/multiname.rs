use std::collections::VecDeque;
use std::iter;

use super::context::Context;
use super::errors::{Expected, ExpectedName};
use super::stream::Stream;
use super::Lexeme;
use crate::error::{ProjectError, ProjectResult};
use crate::utils::iter::{box_chain, box_once, BoxedIterIter};
use crate::Tok;

fn parse_multiname_branch(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<(BoxedIterIter<Tok<String>>, Stream<'_>)> {
  let comma = ctx.interner().i(",");
  let (subnames, cursor) = parse_multiname_rec(cursor, ctx.clone())?;
  let (delim, cursor) = cursor.trim().pop()?;
  match delim.lexeme {
    Lexeme::Name(n) if n == comma => {
      let (tail, cont) = parse_multiname_branch(cursor, ctx)?;
      Ok((box_chain!(subnames, tail), cont))
    },
    Lexeme::RP('(') => Ok((subnames, cursor)),
    _ => Err(
      Expected {
        expected: vec![Lexeme::Name(comma), Lexeme::RP('(')],
        or_name: false,
        found: delim.clone(),
      }
      .rc(),
    ),
  }
}

pub fn parse_multiname_rec(
  curosr: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<(BoxedIterIter<Tok<String>>, Stream<'_>)> {
  let comma = ctx.interner().i(",");
  let (head, cursor) = curosr.trim().pop()?;
  match &head.lexeme {
    Lexeme::LP('(') => parse_multiname_branch(cursor, ctx),
    Lexeme::LP('[') => {
      let (op_ent, cursor) = cursor.trim().pop()?;
      let op = ExpectedName::expect(op_ent)?;
      let (rp_ent, cursor) = cursor.trim().pop()?;
      Expected::expect(Lexeme::RP('['), rp_ent)?;
      Ok((box_once(box_once(op)), cursor))
    },
    Lexeme::Name(n) if *n != comma => {
      let cursor = cursor.trim();
      if cursor.get(0).ok().map(|e| &e.lexeme) == Some(&Lexeme::NS) {
        let cursor = cursor.step()?;
        let (out, cursor) = parse_multiname_rec(cursor, ctx)?;
        let out = Box::new(out.map(|i| box_chain!(i, iter::once(*n))));
        Ok((out, cursor))
      } else {
        Ok((box_once(box_once(*n)), cursor))
      }
    },
    _ => Err(
      Expected {
        expected: vec![Lexeme::LP('(')],
        or_name: true,
        found: head.clone(),
      }
      .rc(),
    ),
  }
}

pub fn parse_multiname(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<(Vec<Vec<Tok<String>>>, Stream<'_>)> {
  let (output, cont) = parse_multiname_rec(cursor, ctx)?;
  let output = output
    .map(|it| {
      let mut deque = VecDeque::with_capacity(it.size_hint().0);
      for item in it {
        deque.push_front(item)
      }
      deque.into()
    })
    .collect();
  Ok((output, cont))
}
