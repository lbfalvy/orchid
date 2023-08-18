use std::collections::VecDeque;

use super::context::Context;
use super::errors::Expected;
use super::stream::Stream;
use super::Lexeme;
use crate::error::{ProjectError, ProjectResult};
use crate::sourcefile::Import;
use crate::utils::iter::{box_chain, box_once};
use crate::utils::BoxedIter;
use crate::Tok;

struct Subresult {
  glob: bool,
  deque: VecDeque<Tok<String>>,
}
impl Subresult {
  fn new_glob() -> Self {
    Self { glob: true, deque: VecDeque::new() }
  }

  fn new_named(name: Tok<String>) -> Self {
    Self { glob: false, deque: VecDeque::from([name]) }
  }

  fn push_front(mut self, name: Tok<String>) -> Self {
    self.deque.push_front(name);
    self
  }

  fn finalize(self) -> Import {
    let Self { mut deque, glob } = self;
    debug_assert!(glob || !deque.is_empty(), "The constructors forbid this");
    let name = if glob { None } else { deque.pop_back() };
    Import { name, path: deque.into() }
  }
}

fn parse_multiname_branch(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<(BoxedIter<Subresult>, Stream<'_>)> {
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

fn parse_multiname_rec(
  curosr: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<(BoxedIter<Subresult>, Stream<'_>)> {
  let star = ctx.interner().i("*");
  let comma = ctx.interner().i(",");
  let (head, mut cursor) = curosr.trim().pop()?;
  match &head.lexeme {
    Lexeme::LP('(') => parse_multiname_branch(cursor, ctx),
    Lexeme::LP('[') => {
      let mut names = Vec::new();
      loop {
        let head;
        (head, cursor) = cursor.trim().pop()?;
        match head.lexeme {
          Lexeme::Name(n) => names.push(n),
          Lexeme::RP('[') => break,
          _ => {
            let err = Expected {
              expected: vec![Lexeme::RP('[')],
              or_name: true,
              found: head.clone(),
            };
            return Err(err.rc());
          },
        }
      }
      Ok((Box::new(names.into_iter().map(Subresult::new_named)), cursor))
    },
    Lexeme::Name(n) if *n == star =>
      Ok((box_once(Subresult::new_glob()), cursor)),
    Lexeme::Name(n) if ![comma, star].contains(n) => {
      let cursor = cursor.trim();
      if cursor.get(0).ok().map(|e| &e.lexeme) == Some(&Lexeme::NS) {
        let cursor = cursor.step()?;
        let (out, cursor) = parse_multiname_rec(cursor, ctx)?;
        let out = Box::new(out.map(|sr| sr.push_front(*n)));
        Ok((out, cursor))
      } else {
        Ok((box_once(Subresult::new_named(*n)), cursor))
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
) -> ProjectResult<(Vec<Import>, Stream<'_>)> {
  let (output, cont) = parse_multiname_rec(cursor, ctx)?;
  Ok((output.map(|sr| sr.finalize()).collect(), cont))
}
