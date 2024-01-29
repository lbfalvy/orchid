//! Convert source text into a sequence of tokens. Newlines and comments are
//! included, but spacing is converted into numerical ranges on the elements.
//!
//! Literals lose their syntax form here and are handled in an abstract
//! representation hence

use std::fmt::Display;
use std::ops::Range;
use std::sync::Arc;

use intern_all::{i, Tok};
use itertools::Itertools;
use ordered_float::NotNan;

use super::context::ParseCtx;
use super::errors::{FloatPlacehPrio, NoCommentEnd};
use super::lex_plugin::LexerPlugin;
use super::numeric::{numstart, parse_num, print_nat16};
use super::string::StringLexer;
use crate::error::ProjectResult;
use crate::foreign::atom::AtomGenerator;
use crate::libs::std::number::Numeric;
use crate::parse::errors::ParseErrorKind;
use crate::parse::lex_plugin::LexPlugReqImpl;
use crate::parse::numeric::{numchar, NumericLexer};
use crate::parse::parsed::{PHClass, PType, Placeholder};

/// A lexeme and the location where it was found
#[derive(Clone, Debug)]
pub struct Entry {
  /// the lexeme
  pub lexeme: Lexeme,
  /// the range in bytes
  pub range: Range<usize>,
}
impl Entry {
  /// Checks if the lexeme is a comment or line break
  #[must_use]
  pub fn is_filler(&self) -> bool {
    matches!(self.lexeme, Lexeme::Comment(_) | Lexeme::BR)
  }

  /// Create a new entry
  #[must_use]
  pub fn new(range: Range<usize>, lexeme: Lexeme) -> Self { Self { lexeme, range } }
}

impl Display for Entry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.lexeme.fmt(f)
  }
}

/// A unit of syntax
#[derive(Clone, Debug)]
pub enum Lexeme {
  /// Atoms parsed by plugins
  Atom(AtomGenerator),
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
      (Self::Atom(a1), Self::Atom(a2)) =>
        a1.run().0.parser_eq(a2.run().0.as_any_ref()),
      (Self::Comment(c1), Self::Comment(c2)) => c1 == c2,
      (Self::LP(p1), Self::LP(p2)) | (Self::RP(p1), Self::RP(p2)) => p1 == p2,
      (Self::Name(n1), Self::Name(n2)) => n1 == n2,
      (Self::Placeh(ph1), Self::Placeh(ph2)) => ph1 == ph2,
      (..) => false,
    }
  }
}

/// Data returned from the lexer
pub struct LexRes<'a> {
  /// Leftover text. If the bail callback never returned true, this is empty
  pub tail: &'a str,
  /// Lexemes extracted from the text
  pub tokens: Vec<Entry>,
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
  !namestart(c) && !numstart(c) && !c.is_whitespace() && !"()[]{},'\"\\".contains(c)
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

static BUILTIN_ATOMS: &[&dyn LexerPlugin] = &[&NumericLexer, &StringLexer];

/// Convert source code to a flat list of tokens. The bail callback will be
/// called between lexemes. When it returns true, the remaining text is
/// returned without processing.
pub fn lex<'a>(
  mut tokens: Vec<Entry>,
  mut data: &'a str,
  ctx: &'_ impl ParseCtx,
  mut bail: impl FnMut(&str) -> ProjectResult<bool>,
) -> ProjectResult<LexRes<'a>> {
  let mut prev_len = data.len() + 1;
  'tail: loop {
    if prev_len == data.len() {
      panic!("got stuck at {data:?}, parsed {:?}", tokens.last().unwrap());
    }
    prev_len = data.len();
    data = data.trim_start_matches(|c: char| c.is_whitespace() && c != '\n');
    if bail(data)? {
      return Ok(LexRes { tokens, tail: data });
    }
    let mut chars = data.chars();
    let head = match chars.next() {
      None => return Ok(LexRes { tokens, tail: data }),
      Some(h) => h,
    };
    let req = LexPlugReqImpl { tail: data, ctx };
    for lexer in ctx.lexers().chain(BUILTIN_ATOMS.iter().copied()) {
      if let Some(res) = lexer.lex(&req) {
        let LexRes { tail, tokens: mut new_tokens } = res?;
        if tail.len() == data.len() {
          panic!("lexer plugin consumed 0 characters")
        }
        tokens.append(&mut new_tokens);
        data = tail;
        continue 'tail;
      }
    }
    for (prefix, lexeme) in lit_table() {
      if let Some(tail) = data.strip_prefix(prefix) {
        tokens.push(Entry::new(ctx.range(prefix.len(), tail), lexeme.clone()));
        data = tail;
        continue 'tail;
      }
    }

    if let Some(tail) = data.strip_prefix(',') {
      let lexeme = Lexeme::Name(i(","));
      tokens.push(Entry::new(ctx.range(1, tail), lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix("--[") {
      let (note, tail) = (tail.split_once("]--"))
        .ok_or_else(|| NoCommentEnd.pack(ctx.source_range(tail.len(), "")))?;
      let lexeme = Lexeme::Comment(Arc::new(note.to_string()));
      tokens.push(Entry::new(ctx.range(note.len() + 3, tail), lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix("--") {
      let (note, tail) = split_filter(tail, |c| c != '\n');
      let lexeme = Lexeme::Comment(Arc::new(note.to_string()));
      tokens.push(Entry::new(ctx.range(note.len(), tail), lexeme));
      data = tail;
      continue 'tail;
    }
    if let Some(tail) = data.strip_prefix('=') {
      if tail.chars().next().map_or(false, numstart) {
        let (num, post_num) = split_filter(tail, numchar);
        if let Some(tail) = post_num.strip_prefix("=>") {
          let prio = parse_num(num)
            .map_err(|e| e.into_proj(num.len(), post_num, ctx))?;
          let lexeme = Lexeme::Arrow(prio.as_float());
          tokens.push(Entry::new(ctx.range(num.len() + 3, tail), lexeme));
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
        let class = if nameonly { PHClass::Name } else { PHClass::Scalar };
        let lexeme = Lexeme::Placeh(Placeholder { name: i(name), class });
        tokens.push(Entry::new(ctx.range(name.len() + 1, tail), lexeme));
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
                .and_then(|num| match num {
                  Numeric::Uint(usize) => Ok(usize),
                  Numeric::Float(_) => Err(
                    FloatPlacehPrio.pack(ctx.source_range(num_str.len(), tail)),
                  ),
                })
                .map(|p| (p, num_str.len() + 1, tail))
            })
            .unwrap_or(Ok((0, 0, tail)))?;
          let byte_len = if nonzero { 4 } else { 3 } + priolen + name.len();
          let class = PHClass::Vec { nonzero, prio };
          let lexeme = Lexeme::Placeh(Placeholder { name: i(name), class });
          tokens.push(Entry::new(ctx.range(byte_len, tail), lexeme));
          data = tail;
          continue 'tail;
        }
      }
    }
    if namestart(head) {
      let (name, tail) = split_filter(data, namechar);
      if !name.is_empty() {
        let lexeme = Lexeme::Name(i(name));
        tokens.push(Entry::new(ctx.range(name.len(), tail), lexeme));
        data = tail;
        continue 'tail;
      }
    }
    if opchar(head) {
      let (name, tail) = split_filter(data, opchar);
      if !name.is_empty() {
        let lexeme = Lexeme::Name(i(name));
        tokens.push(Entry::new(ctx.range(name.len(), tail), lexeme));
        data = tail;
        continue 'tail;
      }
    }
    unreachable!(r#"opchar is pretty much defined as "not namechar" "#)
  }
}
