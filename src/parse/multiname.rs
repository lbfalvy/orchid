//! Parse the tree-like name sets used to represent imports

use std::collections::VecDeque;
use std::ops::Range;

use intern_all::{i, Tok};

use super::context::ParseCtx;
use super::errors::{Expected, ParseErrorKind};
use super::frag::Frag;
use super::lexer::{Entry, Lexeme};
use crate::error::ProjectResult;
use crate::location::SourceRange;
use crate::name::VPath;
use crate::parse::parsed::{Import, PType};
use crate::utils::boxed_iter::{box_chain, box_once, BoxedIter};

struct Subresult {
  glob: bool,
  deque: VecDeque<Tok<String>>,
  range: Range<usize>,
}
impl Subresult {
  #[must_use]
  fn new_glob(range: &Range<usize>) -> Self {
    Self { glob: true, deque: VecDeque::new(), range: range.clone() }
  }

  #[must_use]
  fn new_named(name: Tok<String>, range: &Range<usize>) -> Self {
    Self { glob: false, deque: VecDeque::from([name]), range: range.clone() }
  }

  #[must_use]
  fn push_front(mut self, name: Tok<String>) -> Self {
    self.deque.push_front(name);
    self
  }

  #[must_use]
  fn finalize(self, ctx: &(impl ParseCtx + ?Sized)) -> Import {
    let Self { mut deque, glob, range } = self;
    debug_assert!(glob || !deque.is_empty(), "The constructors forbid this");
    let name = if glob { None } else { deque.pop_back() };
    let range = ctx.range_loc(&range);
    Import { name, range, path: VPath(deque.into()) }
  }
}

fn parse_multiname_branch<'a>(
  cursor: Frag<'a>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<(BoxedIter<'a, Subresult>, Frag<'a>)> {
  let comma = i(",");
  let (subnames, cursor) = parse_multiname_rec(cursor, ctx)?;
  let (Entry { lexeme, range }, cursor) = cursor.trim().pop(ctx)?;
  match &lexeme {
    Lexeme::RP(PType::Par) => Ok((subnames, cursor)),
    Lexeme::Name(n) if n == &comma => {
      let (tail, cont) = parse_multiname_branch(cursor, ctx)?;
      Ok((box_chain!(subnames, tail), cont))
    },
    _ => {
      let expected = vec![Lexeme::Name(comma), Lexeme::RP(PType::Par)];
      let err = Expected { expected, or_name: false, found: lexeme.clone() };
      Err(err.pack(SourceRange { range: range.clone(), code: ctx.code_info() }))
    },
  }
}

fn parse_multiname_rec<'a>(
  cursor: Frag<'a>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<(BoxedIter<'a, Subresult>, Frag<'a>)> {
  let star = i("*");
  let comma = i(",");
  let (head, mut cursor) = cursor.trim().pop(ctx)?;
  match &head.lexeme {
    Lexeme::LP(PType::Par) => parse_multiname_branch(cursor, ctx),
    Lexeme::LP(PType::Sqr) => {
      let mut names = Vec::new();
      loop {
        let (Entry { lexeme, range }, tail) = cursor.trim().pop(ctx)?;
        cursor = tail;
        match lexeme {
          Lexeme::Name(n) => names.push((n.clone(), range)),
          Lexeme::RP(PType::Sqr) => break,
          _ => {
            let err = Expected {
              expected: vec![Lexeme::RP(PType::Sqr)],
              or_name: true,
              found: head.lexeme.clone(),
            };
            return Err(err.pack(ctx.range_loc(range)));
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
      Ok((box_once(Subresult::new_glob(&head.range)), cursor)),
    Lexeme::Name(n) if ![comma, star].contains(n) => {
      let cursor = cursor.trim();
      if cursor.get(0, ctx).map_or(false, |e| e.lexeme.strict_eq(&Lexeme::NS)) {
        let cursor = cursor.step(ctx)?;
        let (out, cursor) = parse_multiname_rec(cursor, ctx)?;
        let out = Box::new(out.map(|sr| sr.push_front(n.clone())));
        Ok((out, cursor))
      } else {
        Ok((box_once(Subresult::new_named(n.clone(), &head.range)), cursor))
      }
    },
    _ => {
      let expected = vec![Lexeme::LP(PType::Par)];
      let err =
        Expected { expected, or_name: true, found: head.lexeme.clone() };
      Err(err.pack(ctx.range_loc(&head.range)))
    },
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
  cursor: Frag<'a>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<(Vec<Import>, Frag<'a>)> {
  let (output, cont) = parse_multiname_rec(cursor, ctx)?;
  Ok((output.map(|sr| sr.finalize(ctx)).collect(), cont))
}
