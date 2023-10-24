use std::collections::VecDeque;

use super::context::Context;
use super::errors::Expected;
use super::stream::Stream;
use super::Lexeme;
use crate::ast::PType;
use crate::error::{ProjectError, ProjectResult};
use crate::sourcefile::Import;
use crate::utils::boxed_iter::{box_chain, box_once};
use crate::utils::BoxedIter;
use crate::{Location, Tok};

struct Subresult {
  glob: bool,
  deque: VecDeque<Tok<String>>,
  location: Location,
}
impl Subresult {
  #[must_use]
  fn new_glob(location: Location) -> Self {
    Self { glob: true, deque: VecDeque::new(), location }
  }

  #[must_use]
  fn new_named(name: Tok<String>, location: Location) -> Self {
    Self { location, glob: false, deque: VecDeque::from([name]) }
  }

  #[must_use]
  fn push_front(mut self, name: Tok<String>) -> Self {
    self.deque.push_front(name);
    self
  }

  #[must_use]
  fn finalize(self) -> Import {
    let Self { mut deque, glob, location } = self;
    debug_assert!(glob || !deque.is_empty(), "The constructors forbid this");
    let name = if glob { None } else { deque.pop_back() };
    Import { name, location, path: deque.into() }
  }
}

fn parse_multiname_branch<'a>(
  cursor: Stream<'a>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<(BoxedIter<'a, Subresult>, Stream<'a>)> {
  let comma = ctx.interner().i(",");
  let (subnames, cursor) = parse_multiname_rec(cursor, ctx)?;
  let (delim, cursor) = cursor.trim().pop()?;
  match &delim.lexeme {
    Lexeme::Name(n) if n == &comma => {
      let (tail, cont) = parse_multiname_branch(cursor, ctx)?;
      Ok((box_chain!(subnames, tail), cont))
    },
    Lexeme::RP(PType::Par) => Ok((subnames, cursor)),
    _ => Err(
      Expected {
        expected: vec![Lexeme::Name(comma), Lexeme::RP(PType::Par)],
        or_name: false,
        found: delim.clone(),
      }
      .rc(),
    ),
  }
}

fn parse_multiname_rec<'a>(
  curosr: Stream<'a>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<(BoxedIter<'a, Subresult>, Stream<'a>)> {
  let star = ctx.interner().i("*");
  let comma = ctx.interner().i(",");
  let (head, mut cursor) = curosr.trim().pop()?;
  match &head.lexeme {
    Lexeme::LP(PType::Par) => parse_multiname_branch(cursor, ctx),
    Lexeme::LP(PType::Sqr) => {
      let mut names = Vec::new();
      loop {
        let head;
        (head, cursor) = cursor.trim().pop()?;
        match &head.lexeme {
          Lexeme::Name(n) => names.push((n, head.location())),
          Lexeme::RP(PType::Sqr) => break,
          _ => {
            let err = Expected {
              expected: vec![Lexeme::RP(PType::Sqr)],
              or_name: true,
              found: head.clone(),
            };
            return Err(err.rc());
          },
        }
      }
      Ok((
        Box::new(names.into_iter().map(|(name, location)| {
          Subresult::new_named(name.clone(), location)
        })),
        cursor,
      ))
    },
    Lexeme::Name(n) if *n == star =>
      Ok((box_once(Subresult::new_glob(head.location())), cursor)),
    Lexeme::Name(n) if ![comma, star].contains(n) => {
      let cursor = cursor.trim();
      if cursor.get(0).map_or(false, |e| e.lexeme.strict_eq(&Lexeme::NS)) {
        let cursor = cursor.step()?;
        let (out, cursor) = parse_multiname_rec(cursor, ctx)?;
        let out = Box::new(out.map(|sr| sr.push_front(n.clone())));
        Ok((out, cursor))
      } else {
        Ok((box_once(Subresult::new_named(n.clone(), head.location())), cursor))
      }
    },
    _ => Err(
      Expected {
        expected: vec![Lexeme::LP(PType::Par)],
        or_name: true,
        found: head.clone(),
      }
      .rc(),
    ),
  }
}

/// Parse a tree that describes several names. The tree can be
/// 
/// - name (except `,` or `*`)
/// - name (except `,` or `*`) `::` tree
/// - `(` tree `,` tree ... `)`
/// - `*` (wildcard)
/// - `[` name name ... `]` (including `,` or `*`).
/// 
/// Examples of valid syntax:
/// 
/// ```txt
/// foo
/// foo::bar::baz
/// foo::bar::(baz, quz::quux, fimble::*)
/// foo::bar::[baz quz * +]
/// ```
pub fn parse_multiname<'a>(
  cursor: Stream<'a>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<(Vec<Import>, Stream<'a>)> {
  let (output, cont) = parse_multiname_rec(cursor, ctx)?;
  Ok((output.map(|sr| sr.finalize()).collect(), cont))
}
