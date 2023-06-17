use std::fmt;
use std::ops::Range;

use chumsky::prelude::*;
use chumsky::text::keyword;
use chumsky::{Parser, Span};
use ordered_float::NotNan;

use super::context::Context;
use super::decls::SimpleParser;
use super::{comment, name, number, placeholder, string};
use crate::ast::{PHClass, Placeholder};
use crate::interner::{InternedDisplay, Interner, Tok};
use crate::representations::Literal;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
  pub lexeme: Lexeme,
  pub range: Range<usize>,
}
impl Entry {
  pub fn is_filler(&self) -> bool {
    matches!(self.lexeme, Lexeme::Comment(_))
      || matches!(self.lexeme, Lexeme::BR)
  }
}

impl InternedDisplay for Entry {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    self.lexeme.fmt_i(f, i)
  }
}

impl From<Entry> for (Lexeme, Range<usize>) {
  fn from(ent: Entry) -> Self {
    (ent.lexeme, ent.range)
  }
}

impl Span for Entry {
  type Context = Lexeme;
  type Offset = usize;

  fn context(&self) -> Self::Context {
    self.lexeme.clone()
  }
  fn start(&self) -> Self::Offset {
    self.range.start()
  }
  fn end(&self) -> Self::Offset {
    self.range.end()
  }
  fn new(context: Self::Context, range: Range<Self::Offset>) -> Self {
    Self { lexeme: context, range }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Lexeme {
  Literal(Literal),
  Name(Tok<String>),
  Rule(NotNan<f64>),
  /// Walrus operator (formerly shorthand macro)
  Const,
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
  Type, // type operator
  Comment(String),
  Export,
  Import,
  Namespace,
  PH(Placeholder),
}

impl InternedDisplay for Lexeme {
  fn fmt_i(
    &self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result {
    match self {
      Self::Literal(l) => write!(f, "{:?}", l),
      Self::Name(token) => write!(f, "{}", i.r(*token)),
      Self::Const => write!(f, ":="),
      Self::Rule(prio) => write!(f, "={}=>", prio),
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
      Self::Export => write!(f, "export"),
      Self::Import => write!(f, "import"),
      Self::Namespace => write!(f, "namespace"),
      Self::PH(Placeholder { name, class }) => match *class {
        PHClass::Scalar => write!(f, "${}", i.r(*name)),
        PHClass::Vec { nonzero, prio } => {
          if nonzero { write!(f, "...") } else { write!(f, "..") }?;
          write!(f, "${}", i.r(*name))?;
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
  pub fn rule(prio: impl Into<f64>) -> Self {
    Lexeme::Rule(NotNan::new(prio.into()).expect("Rule priority cannot be NaN"))
  }

  pub fn parser<E: chumsky::Error<Entry>>(
    self,
  ) -> impl Parser<Entry, Entry, Error = E> + Clone {
    filter(move |ent: &Entry| ent.lexeme == self)
  }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LexedText(pub Vec<Entry>);

impl InternedDisplay for LexedText {
  fn fmt_i(&self, f: &mut fmt::Formatter<'_>, i: &Interner) -> fmt::Result {
    for tok in self.0.iter() {
      tok.fmt_i(f, i)?;
      f.write_str(" ")?
    }
    Ok(())
  }
}

fn paren_parser(lp: char, rp: char) -> impl SimpleParser<char, Lexeme> {
  just(lp).to(Lexeme::LP(lp)).or(just(rp).to(Lexeme::RP(lp)))
}

pub fn literal_parser() -> impl SimpleParser<char, Literal> {
  choice((
    // all ints are valid floats so it takes precedence
    number::int_parser().map(Literal::Uint),
    number::float_parser().map(Literal::Num),
    string::char_parser().map(Literal::Char),
    string::str_parser().map(Literal::Str),
  ))
}

pub static BASE_OPS: &[&str] = &[",", ".", "..", "..."];

pub fn lexer<'a>(
  ctx: impl Context + 'a,
) -> impl SimpleParser<char, Vec<Entry>> + 'a {
  let all_ops = ctx
    .ops()
    .iter()
    .map(|op| op.as_ref())
    .chain(BASE_OPS.iter().cloned())
    .map(str::to_string)
    .collect::<Vec<_>>();
  choice((
    keyword("export").to(Lexeme::Export),
    keyword("module").to(Lexeme::Namespace),
    keyword("import").to(Lexeme::Import),
    paren_parser('(', ')'),
    paren_parser('[', ']'),
    paren_parser('{', '}'),
    just(":=").to(Lexeme::Const),
    just("=")
      .ignore_then(number::float_parser())
      .then_ignore(just("=>"))
      .map(Lexeme::rule),
    comment::comment_parser().map(Lexeme::Comment),
    just("::").to(Lexeme::NS),
    just('\\').to(Lexeme::BS),
    just('@').to(Lexeme::At),
    just(':').to(Lexeme::Type),
    just('\n').to(Lexeme::BR),
    placeholder::placeholder_parser(ctx.clone()).map(Lexeme::PH),
    literal_parser().map(Lexeme::Literal),
    name::name_parser(&all_ops)
      .map(move |n| Lexeme::Name(ctx.interner().i(&n))),
  ))
  .map_with_span(|lexeme, range| Entry { lexeme, range })
  .padded_by(one_of(" \t").repeated())
  .repeated()
  .then_ignore(end())
}

pub fn filter_map_lex<'a, O, M: ToString>(
  f: impl Fn(Lexeme) -> Result<O, M> + Clone + 'a,
) -> impl SimpleParser<Entry, (O, Range<usize>)> + Clone + 'a {
  filter_map(move |s: Range<usize>, e: Entry| {
    let out = f(e.lexeme).map_err(|msg| Simple::custom(s.clone(), msg))?;
    Ok((out, s))
  })
}
