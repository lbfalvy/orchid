//! The [Frag] is the main input datastructure of parsers. Beyond the slice of
//! tokens, it contains a fallback value that can be used for error reporting if
//! the fragment is empty.

use std::ops::Range;

use super::context::ParseCtx;
use super::errors::{ExpectedEOL, NotFound, ParseErrorKind, UnexpectedEOL};
use super::lexer::{Entry, Lexeme};
use crate::error::ProjectResult;

/// Represents a slice which may or may not contain items, and a fallback entry
/// used for error reporting whenever the errant fragment is empty.
#[must_use = "fragment of code should not be discarded implicitly"]
#[derive(Clone, Copy)]
pub struct Frag<'a> {
  /// Entry to place in errors if the fragment contains no tokens
  pub fallback: &'a Entry,
  /// Tokens to parse
  pub data: &'a [Entry],
}
impl<'a> Frag<'a> {
  /// Create a new fragment
  pub fn new(fallback: &'a Entry, data: &'a [Entry]) -> Self {
    Self { fallback, data }
  }

  /// Remove comments and line breaks from both ends of the text
  pub fn trim(self) -> Self {
    let Self { data, fallback } = self;
    let front = data.iter().take_while(|e| e.is_filler()).count();
    let (_, right) = data.split_at(front);
    let back = right.iter().rev().take_while(|e| e.is_filler()).count();
    let (data, _) = right.split_at(right.len() - back);
    Self { fallback, data }
  }

  /// Discard the first entry
  pub fn step(self, ctx: &(impl ParseCtx + ?Sized)) -> ProjectResult<Self> {
    let Self { data, fallback: Entry { lexeme, range } } = self;
    match data.split_first() {
      Some((fallback, data)) => Ok(Frag { data, fallback }),
      None => Err(UnexpectedEOL(lexeme.clone()).pack(ctx.range_loc(range))),
    }
  }

  /// Get the first entry
  pub fn pop(
    self,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectResult<(&'a Entry, Self)> {
    Ok((self.get(0, ctx)?, self.step(ctx)?))
  }

  /// Retrieve an index from a slice or raise an [UnexpectedEOL].
  pub fn get(
    self,
    idx: usize,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectResult<&'a Entry> {
    self.data.get(idx).ok_or_else(|| {
      let entry = self.data.last().unwrap_or(self.fallback).clone();
      UnexpectedEOL(entry.lexeme).pack(ctx.range_loc(&entry.range))
    })
  }

  /// Area covered by this fragment
  #[must_use]
  pub fn range(self) -> Range<usize> {
    self.data.first().map_or_else(
      || self.fallback.range.clone(),
      |f| f.range.start..self.data.last().unwrap().range.end,
    )
  }

  /// Find a given token, split the fragment there and read some value from the
  /// separator. See also [fragment::find]
  pub fn find_map<T>(
    self,
    msg: &'static str,
    ctx: &(impl ParseCtx + ?Sized),
    mut f: impl FnMut(&'a Lexeme) -> Option<T>,
  ) -> ProjectResult<(Self, T, Self)> {
    let Self { data, fallback } = self;
    let (dot_idx, output) = skip_parenthesized(data.iter())
      .find_map(|(i, e)| f(&e.lexeme).map(|t| (i, t)))
      .ok_or_else(|| NotFound(msg).pack(ctx.range_loc(&self.range())))?;
    let (left, not_left) = data.split_at(dot_idx);
    let (middle_ent, right) = not_left.split_first().unwrap();
    Ok((Self::new(fallback, left), output, Self::new(middle_ent, right)))
  }

  /// Split the fragment at a token and return just the two sides.
  /// See also [fragment::find_map].
  pub fn find(
    self,
    descr: &'static str,
    ctx: &(impl ParseCtx + ?Sized),
    mut f: impl FnMut(&Lexeme) -> bool,
  ) -> ProjectResult<(Self, Self)> {
    let (l, _, r) = self.find_map(descr, ctx, |l| Some(l).filter(|l| f(l)))?;
    Ok((l, r))
  }

  /// Remove the last item from the fragment
  pub fn pop_back(
    self,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectResult<(&'a Entry, Self)> {
    let Self { data, fallback } = self;
    let (last, data) = (data.split_last()).ok_or_else(|| {
      UnexpectedEOL(fallback.lexeme.clone())
        .pack(ctx.range_loc(&fallback.range))
    })?;
    Ok((last, Self { fallback, data }))
  }

  /// # Panics
  ///
  /// If the slice is empty
  pub fn from_slice(data: &'a [Entry]) -> Self {
    let fallback =
      (data.first()).expect("Empty slice cannot be converted into a parseable");
    Self { data, fallback }
  }

  /// Assert that the fragment is empty.
  pub fn expect_empty(
    self,
    ctx: &(impl ParseCtx + ?Sized),
  ) -> ProjectResult<()> {
    match self.data.first() {
      Some(x) => Err(ExpectedEOL.pack(ctx.range_loc(&x.range))),
      None => Ok(()),
    }
  }
}

fn skip_parenthesized<'a>(
  it: impl Iterator<Item = &'a Entry>,
) -> impl Iterator<Item = (usize, &'a Entry)> {
  let mut paren_lvl = 1;
  it.enumerate().filter(move |(_, e)| {
    match e.lexeme {
      Lexeme::LP(_) => paren_lvl += 1,
      Lexeme::RP(_) => paren_lvl -= 1,
      _ => (),
    }
    paren_lvl <= 1
  })
}
