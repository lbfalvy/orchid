use std::fmt::Display;
use std::ops::Range;
use std::sync::Arc;

use itertools::Itertools;
use ordered_float::NotNan;

use super::LexerPlugin;
use super::context::Context;
use super::errors::{FloatPlacehPrio, NoCommentEnd};
use super::numeric::{parse_num, print_nat16, numstart};
use crate::ast::{PHClass, Placeholder};
use crate::error::{ProjectResult, ProjectError};
use crate::foreign::Atom;
use crate::interner::Tok;
use crate::parse::numeric::{numchar, lex_numeric};
use crate::parse::string::lex_string;
use crate::systems::stl::Numeric;
use crate::utils::pure_seq::next;
use crate::utils::unwrap_or;
use crate::{Location, VName};

#[derive(Clone, Debug)]
pub struct Entry {
  pub lexeme: Lexeme,
  pub location: Location,
}
impl Entry {
  /// Checks if the lexeme is a comment or line break
  #[must_use]
  pub fn is_filler(&self) -> bool {
    matches!(self.lexeme, Lexeme::Comment(_) | Lexeme::BR)
  }

  #[must_use]
  pub fn is_keyword(&self) -> bool {
    false
    // matches!(
    //   self.lexeme,
    //   Lexeme::Const
    //     | Lexeme::Export
    //     | Lexeme::Import
    //     | Lexeme::Macro
    //     | Lexeme::Module
    // )
  }

  #[must_use]
  pub fn location(&self) -> Location { self.location.clone() }

  #[must_use]
  pub fn range(&self) -> Range<usize> {
    self.location.range().expect("An Entry can only have a known location")
  }

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

#[derive(Clone, Debug)]
pub enum Lexeme {
  Atom(Atom),
  Name(Tok<String>),
  Arrow(NotNan<f64>),
  /// Walrus operator (formerly shorthand macro)
  Walrus,
  /// Line break
  BR,
  /// Namespace separator
  NS,
  /// Left paren
  LP(char),
  /// Right paren
  RP(char),
  /// Backslash
  BS,
  At,
  // Dot,
  Type, // type operator
  Comment(Arc<String>),
  // Export,
  // Import,
  // Module,
  // Macro,
  // Const,
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
      Self::LP(l) => write!(f, "{}", l),
      Self::RP(l) => match l {
        '(' => write!(f, ")"),
        '[' => write!(f, "]"),
        '{' => write!(f, "}}"),
        _ => f.debug_tuple("RP").field(l).finish(),
      },
      Self::BR => writeln!(f),
      Self::BS => write!(f, "\\"),
      Self::At => write!(f, "@"),
      Self::Type => write!(f, ":"),
      Self::Comment(text) => write!(f, "--[{}]--", text),
      // Self::Export => write!(f, "export"),
      // Self::Import => write!(f, "import"),
      // Self::Module => write!(f, "module"),
      // Self::Const => write!(f, "const"),
      // Self::Macro => write!(f, "macro"),
      Self::Placeh(Placeholder { name, class }) => match *class {
        PHClass::Scalar => write!(f, "${}", **name),
        PHClass::Vec { nonzero, prio } => {
          if nonzero { write!(f, "...") } else { write!(f, "..") }?;
          write!(f, "${}", **name)?;
          if prio != 0 {
            write!(f, ":{}", prio)?;
          };
          Ok(())
        },
      },
    }
  }
}

impl Lexeme {
  #[must_use]
  pub fn rule(prio: impl Into<f64>) -> Self {
    Lexeme::Arrow(
      NotNan::new(prio.into()).expect("Rule priority cannot be NaN"),
    )
  }

  pub fn strict_eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Arrow(f1), Self::Arrow(f2)) => f1 == f2,
      (Self::At, Self::At) | (Self::BR, Self::BR) => true,
      (Self::BS, Self::BS) /*| (Self::Const, Self::Const)*/ => true,
      // (Self::Export, Self::Export) | (Self::Import, Self::Import) => true,
      // (Self::Macro, Self::Macro) | (Self::Module, Self::Module) => true,
      (Self::NS, Self::NS) | (Self::Type, Self::Type) => true,
      (Self::Walrus, Self::Walrus) => true,
      (Self::Atom(a1), Self::Atom(a2)) => a1.0.strict_eq(&a2.0),
      (Self::Comment(c1), Self::Comment(c2)) => c1 == c2,
      (Self::LP(p1), Self::LP(p2)) | (Self::RP(p1), Self::RP(p2)) => p1 == p2,
      (Self::Name(n1), Self::Name(n2)) => n1 == n2,
      (Self::Placeh(ph1), Self::Placeh(ph2)) => ph1 == ph2,
      (_, _) => false,
    }
  }
}

#[allow(unused)]
pub fn format(lexed: &[Entry]) -> String { lexed.iter().join(" ") }

pub fn namechar(c: char) -> bool { c.is_alphanumeric() | (c == '_') }
pub fn namestart(c: char) -> bool { c.is_alphabetic() | (c == '_') }
pub fn opchar(c: char) -> bool {
  !namestart(c) && !numstart(c) && !c.is_whitespace() && !"()[]{},".contains(c)
}

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
    ("(", Lexeme::LP('(')),
    ("[", Lexeme::LP('[')),
    ("{", Lexeme::LP('{')),
    (")", Lexeme::RP('(')),
    ("]", Lexeme::RP('[')),
    ("}", Lexeme::RP('{')),
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
  'tail:loop {
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
        tokens.push(Entry::new(ctx.location(prefix.len(), tail), lexeme.clone()));
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
          let lexeme = Lexeme::Arrow(parse_num(num).map_err(|e| e.into_proj(num.len(), post_num, ctx))?.as_float());
          let location = ctx.location(num.len() + 3, tail);
          tokens.push(Entry::new(location, lexeme));
          data = tail;
          continue 'tail;
        }
      }
    }
    // todo: parse placeholders, don't forget vectorials!
    if let Some(tail) = data.strip_prefix('$') {
      let (name, tail) = split_filter(tail, namechar);
      if !name.is_empty() {
        let name = ctx.interner().i(name);
        let location = ctx.location(name.len() + 1, tail);
        let lexeme = Lexeme::Placeh(Placeholder { name, class: PHClass::Scalar });
        tokens.push(Entry::new(location, lexeme));
        data = tail;
        continue 'tail;
      }
    }
    if let Some(vec) = data.strip_prefix("..") {
      let (nonzero, tail) =
        vec.strip_prefix('.').map_or((false, vec), |t| (true, t));
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
                    return Err(FloatPlacehPrio(ctx.location(num_str.len(), tail)).rc())
                  }))
                })
                .map(|p| (p, num_str.len() + 1, tail))
            })
            .unwrap_or(Ok((0, 0, tail)))?;
          let byte_len =  if nonzero { 4 } else { 3 } + priolen + name.len();
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
