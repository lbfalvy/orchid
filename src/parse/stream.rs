use super::errors::{ExpectedEOL, NotFound, UnexpectedEOL};
use super::{Entry, Lexeme};
use crate::error::{ProjectError, ProjectResult};
use crate::Location;

/// Represents a slice which may or may not contain items, and a fallback entry
/// used for error reporting whenever the errant stream is empty.
#[must_use = "streams represent segments of code that must be parsed"]
#[derive(Clone, Copy)]
pub struct Stream<'a> {
  /// Entry to place in errors if the stream contains no tokens
  pub fallback: &'a Entry,
  /// Tokens to parse
  pub data: &'a [Entry],
}
impl<'a> Stream<'a> {
  /// Create a new stream
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
  pub fn step(self) -> ProjectResult<Self> {
    let (fallback, data) = (self.data.split_first())
      .ok_or_else(|| UnexpectedEOL { entry: self.fallback.clone() }.rc())?;
    Ok(Stream { data, fallback })
  }

  /// Get the first entry
  pub fn pop(self) -> ProjectResult<(&'a Entry, Stream<'a>)> {
    Ok((self.get(0)?, self.step()?))
  }

  /// Retrieve an index from a slice or raise an [UnexpectedEOL].
  pub fn get(self, idx: usize) -> ProjectResult<&'a Entry> {
    self.data.get(idx).ok_or_else(|| {
      let entry = self.data.last().unwrap_or(self.fallback).clone();
      UnexpectedEOL { entry }.rc()
    })
  }

  /// Area covered by this stream
  #[must_use]
  pub fn location(self) -> Location {
    self.data.first().map_or_else(
      || self.fallback.location(),
      |f| f.location().to(self.data.last().unwrap().location()),
    )
  }

  /// Find a given token, split the stream there and read some value from the
  /// separator. See also [Stream::find]
  pub fn find_map<T>(
    self,
    expected: &'static str,
    mut f: impl FnMut(&'a Lexeme) -> Option<T>,
  ) -> ProjectResult<(Self, T, Self)> {
    let Self { data, fallback } = self;
    let (dot_idx, output) = skip_parenthesized(data.iter())
      .find_map(|(i, e)| f(&e.lexeme).map(|t| (i, t)))
      .ok_or_else(|| NotFound { expected, location: self.location() }.rc())?;
    let (left, not_left) = data.split_at(dot_idx);
    let (middle_ent, right) = not_left.split_first().unwrap();
    Ok((Self::new(fallback, left), output, Self::new(middle_ent, right)))
  }

  /// Split the stream at a token and return just the two sides.
  /// See also [Stream::find_map].
  pub fn find(
    self,
    expected: &'static str,
    mut f: impl FnMut(&Lexeme) -> bool,
  ) -> ProjectResult<(Self, Self)> {
    let (left, _, right) =
      self.find_map(expected, |l| if f(l) { Some(()) } else { None })?;
    Ok((left, right))
  }

  /// Remove the last item from the stream
  pub fn pop_back(self) -> ProjectResult<(&'a Entry, Self)> {
    let Self { data, fallback } = self;
    let (last, data) = (data.split_last())
      .ok_or_else(|| UnexpectedEOL { entry: fallback.clone() }.rc())?;
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

  /// Assert that the stream is empty.
  pub fn expect_empty(self) -> ProjectResult<()> {
    if let Some(x) = self.data.first() {
      Err(ExpectedEOL { location: x.location() }.rc())
    } else {
      Ok(())
    }
  }
}

pub fn skip_parenthesized<'a>(
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
