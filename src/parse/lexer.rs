use std::fmt::Display;
use std::ops::Range;
use std::sync::Arc;

use itertools::Itertools;
use ordered_float::NotNan;

use super::context::Context;
use super::errors::{FloatPlacehPrio, NoCommentEnd};
use super::numeric::{numstart, parse_num, print_nat16};
use super::LexerPlugin;
use crate::ast::{PHClass, PType, Placeholder};
use crate::error::{ProjectError, ProjectResult};
use crate::foreign::Atom;
use crate::interner::Tok;
use crate::parse::numeric::{lex_numeric, numchar};
use crate::parse::string::lex_string;
use crate::systems::stl::Numeric;
use crate::utils::pure_seq::next;
use crate::utils::unwrap_or;
use crate::{Location, VName};

/// A lexeme and the location where it was found
#[derive(Clone, Debug)]
pub struct Entry {
  /// the lexeme
  pub lexeme: Lexeme,
  /// the location. Always a range
  pub location: Location,
}
impl Entry {
  /// Checks if the lexeme is a comment or line break
  #[must_use]
  pub fn is_filler(&self) -> bool {
    matches!(self.lexeme, Lexeme::Comment(_) | Lexeme::BR)
  }

  /// Get location
  #[must_use]
  pub fn location(&self) -> Location { self.location.clone() }

  /// Get range from location
  #[must_use]
  pub fn range(&self) -> Range<usize> {
    self.location.range().expect("An Entry can only have a known location")
  }

  /// Get file path from location
  #[must_use]
  pub fn file(&self) -> Arc<VName> {
    self.location.file().expect("An Entry can only have a range location")
  }

  fn new(location: Location, lexeme: Lexeme) -> Self {
    Self { lexeme, location }
  }
}

impl Display for Entry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.lexeme.fmt(f)
  }
}

impl AsRef<Location> for Entry {
  fn as_ref(&self) -> &Location { &self.location }
}

/// A unit of syntax
#[derive(Clone, Debug)]
pub enum Lexeme {
  /// Atoms parsed by plugins
  Atom(Atom),
  /// Keyword or name
  Name(Tok<String>),
  /// Macro operator `=`number`=>`
  Arrow(NotNan<f64>),
  /// `:=`
  Walrus,
  /// Line break
  BR,
  /// `::`
  NS,
  /// Left paren `([{`
  LP(PType),
  /// Right paren `)]}`
  RP(PType),
  /// `\`
  BS,
  /// `@``
  At,
  /// `:`
  Type,
  /// comment
  Comment(Arc<String>),
  /// placeholder in a macro.
  Placeh(Placeholder),
}

impl Display for Lexeme {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Name(token) => write!(f, "{}", **token),
      Self::Walrus => write!(f, ":="),
      Self::Arrow(prio) => write!(f, "={}=>", print_nat16(*prio)),
      Self::NS => write!(f, "::"),
      Self::LP(t) => write!(f, "{}", t.l()),
      Self::RP(t) => write!(f, "{}", t.r()),
      Self::BR => writeln!(f),
      Self::BS => write!(f, "\\"),
      Self::At => write!(f, "@"),
      Self::Type => write!(f, ":"),
      Self::Comment(text) => write!(f, "--[{}]--", text),
      Self::Placeh(ph) => write!(f, "{ph}"),
    }
  }
}

impl Lexeme {
  /// Compare lexemes for equality. It's `strict` because for atoms it uses the
  /// strict equality comparison
  pub fn strict_eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Arrow(f1), Self::Arrow(f2)) => f1 == f2,
      (Self::At, Self::At) | (Self::BR, Self::BR) => true,
      (Self::BS, Self::BS) => true,
      (Self::NS, Self::NS) | (Self::Type, Self::Type) => true,
      (Self::Walrus, Self::Walrus) => true,
      (Self::Atom(a1), Self::Atom(a2)) => a1.0.strict_eq(&a2.0),
      (Self::Comment(c1), Self::Comment(c2)) => c1 == c2,
      (Self::LP(p1), Self::LP(p2)) | (Self::RP(p1), Self::RP(p2)) => p1 == p2,
      (Self::Name(n1), Self::Name(n2)) => n1 == n2,
      (Self::Placeh(ph1), Self::Placeh(ph2)) => ph1 == ph2,
      (..) => false,
    }
  }
}

/// Neatly format source code
#[allow(unused)]
pub fn format(lexed: &[Entry]) -> String { lexed.iter().join(" ") }

/// Character filter that can appear in a keyword or name
pub fn namechar(c: char) -> bool { c.is_alphanumeric() | (c == '_') }
/// Character filter that can start a name
pub fn namestart(c: char) -> bool { c.is_alphabetic() | (c == '_') }
/// Character filter that can appear in operators.
pub fn opchar(c: char) -> bool {
  !namestart(c) && !numstart(c) && !c.is_whitespace() && !"()[]{},".contains(c)
}

/// Split off all characters from the beginning that match a filter
pub fn split_filter(
  s: &str,
  mut pred: impl FnMut(char) -> bool,
) -> (&str, &str) {
  s.find(|c| !pred(c)).map_or((s, ""), |i| s.split_at(i))
}

fn lit_table() -> impl IntoIterator<Item = (&'static str, Lexeme)> {
  [
    ("\\", Lexeme::BS),
    ("@", Lexeme::At),
    ("(", Lexeme::LP(PType::Par)),
    ("[", Lexeme::LP(PType::Sqr)),
    ("{", Lexeme::LP(PType::Curl)),
    (")", Lexeme::RP(PType::Par)),
    ("]", Lexeme::RP(PType::Sqr)),
    ("}", Lexeme::RP(PType::Curl)),
    ("\n", Lexeme::BR),
    (":=", Lexeme::Walrus),
    ("::", Lexeme::NS),
    (":", Lexeme::Type),
  ]
}

static BUILTIN_ATOMS: &[&dyn LexerPlugin] = &[&lex_string, &lex_numeric];

pub fn lex(
  mut tokens: Vec<Entry>,
  mut data: &str,
  ctx: &impl Context,
) -> ProjectResult<Vec<Entry>> {
  let mut prev_len = data.len() + 1;
  'tail: loop {
    if prev_len == data.len() {
      panic!("got stuck at {data:?}, parsed {:?}", tokens.last().unwrap());
    }
    prev_len = data.len();
    data = data.trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
    let (head, _) = match next(data.chars()) {
      Some((h, t)) => (h, t.as_str()),
      None => return Ok(tokens),
    };
    for lexer in ctx.lexers().iter().chain(BUILTIN_ATOMS.iter()) {
      if let Some(res) = lexer(data, ctx) {
        let (atom, tail) = res?;
        if tail.len() == data.len() {
          panic!("lexer plugin consumed 0 characters")
        }
        let loc = ctx.location(data.len() - tail.len(), tail);
        tokens.push(Entry::new(loc, Lexeme::Atom(atom)));
        data = tail;
        continue 'tail;
      }
    }
    for (prefix, lexeme) in lit_table() {
      if let Some(tail) = data.strip_prefix(prefix) {
        tokens
          .push(Entry::new(ctx.location(prefix.len(), tail), lexeme.clone()));
        data = tail;
        continue 'tail;
      }
    }

    if let Some(tail) = data.strip_prefix(',') {
      let lexeme = Lexeme::Name(ctx.interner().i(","));
      tokens.push(Entry::new(ctx.location(1, tail), lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix("--[") {
      let (note, tail) = (tail.split_once("]--"))
        .ok_or_else(|| NoCommentEnd(ctx.location(tail.len(), "")).rc())?;
      let lexeme = Lexeme::Comment(Arc::new(note.to_string()));
      let location = ctx.location(note.len() + 3, tail);
      tokens.push(Entry::new(location, lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix("--") {
      let (note, tail) = split_filter(tail, |c| c != '\n');
      let lexeme = Lexeme::Comment(Arc::new(note.to_string()));
      let location = ctx.location(note.len(), tail);
      tokens.push(Entry::new(location, lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix('=') {
      if tail.chars().next().map_or(false, numstart) {
        let (num, post_num) = split_filter(tail, numchar);
        if let Some(tail) = post_num.strip_prefix("=>") {
          let lexeme = Lexeme::Arrow(
            parse_num(num)
              .map_err(|e| e.into_proj(num.len(), post_num, ctx))?
              .as_float(),
          );
          let location = ctx.location(num.len() + 3, tail);
          tokens.push(Entry::new(location, lexeme));
          data = tail;
          continue 'tail;
        }
      }
    }
    // todo: parse placeholders, don't forget vectorials!
    if let Some(tail) = data.strip_prefix('$') {
      let (nameonly, tail) =
        tail.strip_prefix('_').map_or((false, tail), |t| (true, t));
      let (name, tail) = split_filter(tail, namechar);
      if !name.is_empty() {
        let name = ctx.interner().i(name);
        let location = ctx.location(name.len() + 1, tail);
        let class = if nameonly { PHClass::Name } else { PHClass::Scalar };
        let lexeme = Lexeme::Placeh(Placeholder { name, class });
        tokens.push(Entry::new(location, lexeme));
        data = tail;
        continue 'tail;
      }
    }
    if let Some(tail) = data.strip_prefix("..") {
      let (nonzero, tail) =
        tail.strip_prefix('.').map_or((false, tail), |t| (true, t));
      if let Some(tail) = tail.strip_prefix('$') {
        let (name, tail) = split_filter(tail, namechar);
        if !name.is_empty() {
          let (prio, priolen, tail) = tail
            .strip_prefix(':')
            .map(|tail| split_filter(tail, numchar))
            .filter(|(num, _)| !num.is_empty())
            .map(|(num_str, tail)| {
              parse_num(num_str)
                .map_err(|e| e.into_proj(num_str.len(), tail, ctx))
                .and_then(|num| {
                  Ok(unwrap_or!(num => Numeric::Uint; {
                    let location = ctx.location(num_str.len(), tail);
                    return Err(FloatPlacehPrio(location).rc())
                  }))
                })
                .map(|p| (p, num_str.len() + 1, tail))
            })
            .unwrap_or(Ok((0, 0, tail)))?;
          let byte_len = if nonzero { 4 } else { 3 } + priolen + name.len();
          let name = ctx.interner().i(name);
          let class = PHClass::Vec { nonzero, prio };
          let lexeme = Lexeme::Placeh(Placeholder { name, class });
          tokens.push(Entry::new(ctx.location(byte_len, tail), lexeme));
          data = tail;
          continue 'tail;
        }
      }
    }
    if namestart(head) {
      let (name, tail) = split_filter(data, namechar);
      if !name.is_empty() {
        let lexeme = Lexeme::Name(ctx.interner().i(name));
        tokens.push(Entry::new(ctx.location(name.len(), tail), lexeme));
        data = tail;
        continue 'tail;
      }
    }
    if opchar(head) {
      let (name, tail) = split_filter(data, opchar);
      if !name.is_empty() {
        let lexeme = Lexeme::Name(ctx.interner().i(name));
        tokens.push(Entry::new(ctx.location(name.len(), tail), lexeme));
        data = tail;
        continue 'tail;
      }
    }
    unreachable!(r#"opchar is pretty much defined as "not namechar" "#)
  }
}
