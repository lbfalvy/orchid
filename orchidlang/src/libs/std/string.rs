//! `std::string` String processing

use std::fmt;
use std::fmt::Write as _;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use intern_all::{i, Tok};
use itertools::Itertools;
use unicode_segmentation::UnicodeSegmentation;

use super::protocol::{gen_resolv, Protocol};
use super::runtime_error::RuntimeError;
use crate::error::{ProjectErrorObj, ProjectResult};
use crate::foreign::atom::{AtomGenerator, Atomic};
use crate::foreign::error::RTResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::{TryFromExpr, WithLoc};
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::{Clause, Expr};
use crate::location::CodeLocation;
use crate::parse::context::ParseCtx;
use crate::parse::errors::ParseErrorKind;
use crate::parse::lex_plugin::{LexPluginRecur, LexPluginReq, LexerPlugin};
use crate::parse::lexer::{Entry, LexRes, Lexeme};
use crate::parse::parsed::PType;
use crate::utils::iter_find::iter_find;

/// An Orchid string which may or may not be interned
#[derive(Clone, Eq)]
pub enum OrcString {
  /// An interned string. Equality-conpared by reference.
  Interned(Tok<String>),
  /// An uninterned bare string. Equality-compared by character
  Runtime(Arc<String>),
}

impl fmt::Debug for OrcString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Runtime(s) => write!(f, "r\"{s}\""),
      Self::Interned(t) => write!(f, "i\"{t}\""),
    }
  }
}

impl OrcString {
  /// Intern the contained string
  pub fn intern(&mut self) {
    if let Self::Runtime(t) = self {
      *self = Self::Interned(i(t.as_str()))
    }
  }
  /// Clone out the plain Rust [String]
  #[must_use]
  pub fn get_string(self) -> String {
    match self {
      Self::Interned(s) => s.as_str().to_owned(),
      Self::Runtime(rc) => Arc::unwrap_or_clone(rc),
    }
  }
}

impl Deref for OrcString {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    match self {
      Self::Interned(t) => t,
      Self::Runtime(r) => r,
    }
  }
}

impl Hash for OrcString {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

impl From<String> for OrcString {
  fn from(value: String) -> Self { Self::Runtime(Arc::new(value)) }
}

impl From<&str> for OrcString {
  fn from(value: &str) -> Self { Self::from(value.to_string()) }
}

impl From<Tok<String>> for OrcString {
  fn from(value: Tok<String>) -> Self { Self::Interned(value) }
}

impl PartialEq for OrcString {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Interned(t1), Self::Interned(t2)) => t1 == t2,
      _ => **self == **other,
    }
  }
}

impl InertPayload for OrcString {
  const TYPE_STR: &'static str = "OrcString";
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}

impl ToClause for String {
  fn to_clause(self, _: CodeLocation) -> Clause { Inert(OrcString::from(self)).atom_cls() }
}

impl TryFromExpr for String {
  fn from_expr(exi: Expr) -> RTResult<Self> {
    Ok(exi.downcast::<Inert<OrcString>>()?.0.get_string())
  }
}

pub(super) fn str_lib() -> ConstTree {
  ConstTree::ns("std::string", [ConstTree::tree([
    // String conversion protocol implementable by external types
    ("conversion", Protocol::tree([], [])),
    xfn_ent("slice", [|s: Inert<OrcString>, i: Inert<usize>, len: Inert<usize>| {
      let graphs = s.0.as_str().graphemes(true);
      if i.0 == 0 {
        return Ok(graphs.take(len.0).collect::<String>());
      }
      let mut prefix = graphs.skip(i.0 - 1);
      if prefix.next().is_none() {
        return Err(RuntimeError::ext(
          "Character index out of bounds".to_string(),
          "indexing string",
        ));
      }
      let mut count = 0;
      let ret = (prefix.take(len.0))
        .map(|x| {
          count += 1;
          x
        })
        .collect::<String>();
      if count == len.0 {
        Ok(ret)
      } else {
        RuntimeError::fail("Character index out of bounds".to_string(), "indexing string")
      }
    }]),
    xfn_ent("concat", [|a: String, b: Inert<OrcString>| a + b.0.as_str()]),
    xfn_ent("find", [|haystack: Inert<OrcString>, needle: Inert<OrcString>| {
      let haystack_graphs = haystack.0.as_str().graphemes(true);
      iter_find(haystack_graphs, needle.0.as_str().graphemes(true)).map(Inert)
    }]),
    xfn_ent("split", [|s: String, i: Inert<usize>| -> (String, String) {
      let mut graphs = s.as_str().graphemes(true);
      (graphs.by_ref().take(i.0).collect(), graphs.collect())
    }]),
    xfn_ent("len", [|s: Inert<OrcString>| Inert(s.0.graphemes(true).count())]),
    xfn_ent("size", [|s: Inert<OrcString>| Inert(s.0.as_bytes().len())]),
    xfn_ent("intern", [|s: Inert<OrcString>| {
      Inert(match s.0 {
        OrcString::Runtime(s) => OrcString::Interned(i(&*s)),
        x => x,
      })
    }]),
    xfn_ent("convert", [|WithLoc(loc, a): WithLoc<Expr>| match a.clone().downcast() {
      Ok(str) => Inert::<OrcString>::atom_expr(str, loc),
      Err(_) => match a.clause.request::<OrcString>() {
        Some(str) => Inert(str).atom_expr(loc),
        None => tpl::a2(gen_resolv("std::string::conversion"), tpl::Slot, tpl::Slot)
          .template(nort_gen(loc), [a.clone(), a]),
      },
    }]),
  ])])
}

/// Reasons why [parse_string] might fail. See [StringError]
enum StringErrorKind {
  /// A unicode escape sequence wasn't followed by 4 hex digits
  NotHex,
  /// A unicode escape sequence contained an unassigned code point
  BadCodePoint,
  /// An unrecognized escape sequence was found
  BadEscSeq,
}

/// Error produced by [parse_string]
struct StringError {
  /// Character where the error occured
  pos: usize,
  /// Reason for the error
  kind: StringErrorKind,
}

impl StringError {
  /// Convert into project error for reporting
  pub fn into_proj(self, ctx: &dyn ParseCtx, pos: usize) -> ProjectErrorObj {
    let start = pos + self.pos;
    let location = ctx.range_loc(&(start..start + 1));
    match self.kind {
      StringErrorKind::NotHex => NotHex.pack(location),
      StringErrorKind::BadCodePoint => BadCodePoint.pack(location),
      StringErrorKind::BadEscSeq => BadEscapeSequence.pack(location),
    }
  }
}

/// Process escape sequences in a string literal
fn parse_string(str: &str) -> Result<String, StringError> {
  let mut target = String::new();
  let mut iter = str.char_indices();
  while let Some((_, c)) = iter.next() {
    if c != '\\' {
      target.push(c);
      continue;
    }
    let (mut pos, code) = iter.next().expect("lexer would have continued");
    let next = match code {
      c @ ('\\' | '/' | '"') => c,
      'b' => '\x08',
      'f' => '\x0f',
      'n' => '\n',
      'r' => '\r',
      't' => '\t',
      '\n' => 'skipws: loop {
        match iter.next() {
          None => return Ok(target),
          Some((_, c)) =>
            if !c.is_whitespace() {
              break 'skipws c;
            },
        }
      },
      'u' => {
        let acc = ((0..4).rev())
          .map(|radical| {
            let (j, c) = (iter.next()).ok_or(StringError { pos, kind: StringErrorKind::NotHex })?;
            pos = j;
            let b = u32::from_str_radix(&String::from(c), 16)
              .map_err(|_| StringError { pos, kind: StringErrorKind::NotHex })?;
            Ok(16u32.pow(radical) + b)
          })
          .fold_ok(0, u32::wrapping_add)?;
        char::from_u32(acc).ok_or(StringError { pos, kind: StringErrorKind::BadCodePoint })?
      },
      _ => return Err(StringError { pos, kind: StringErrorKind::BadEscSeq }),
    };
    target.push(next);
  }
  Ok(target)
}

/// [LexerPlugin] for a string literal that supports interpolateion.
#[derive(Clone)]
pub struct StringLexer;
impl LexerPlugin for StringLexer {
  fn lex<'a>(&self, req: &'_ dyn LexPluginReq<'a>) -> Option<ProjectResult<LexRes<'a>>> {
    req.tail().strip_prefix('\"').map(|mut txt| {
      let ctx = req.ctx();
      let mut parts = vec![Entry::new(ctx.range(0, txt), Lexeme::LP(PType::Par))];
      let mut str = String::new();
      let commit_str = |str: &mut String, tail: &str, parts: &mut Vec<Entry>| -> ProjectResult<_> {
        let str_val = parse_string(str).unwrap_or_else(|e| {
          ctx.reporter().report(e.into_proj(ctx, ctx.pos(txt)));
          String::new()
        });
        let ag = AtomGenerator::cloner(Inert(OrcString::from(i(&str_val))));
        parts.push(Entry::new(ctx.range(str.len(), tail), Lexeme::Atom(ag)));
        *str = String::new();
        Ok(())
      };
      loop {
        if let Some(rest) = txt.strip_prefix('"') {
          commit_str(&mut str, txt, &mut parts)?;
          parts.push(Entry::new(ctx.range(0, rest), Lexeme::RP(PType::Par)));
          if parts.len() == 3 {
            return Ok(LexRes { tail: rest, tokens: vec![parts[1].clone()] });
          }
          return Ok(LexRes { tail: rest, tokens: parts });
        }
        if let Some(rest) = txt.strip_prefix("${") {
          let mut depth = 0;
          commit_str(&mut str, rest, &mut parts)?;
          parts.extend(req.insert("++ std::string::convert (", ctx.source_range(0, rest)));
          let res = req.recurse(LexPluginRecur {
            tail: rest,
            exit: &mut |c| {
              match c.chars().next() {
                None => return Err(UnclosedInterpolation.pack(ctx.source_range(2, rest))),
                Some('{') => depth += 1,
                Some('}') if depth == 0 => return Ok(true),
                Some('}') => depth -= 1,
                _ => (),
              }
              Ok(false)
            },
          })?;
          txt = &res.tail[1..]; // account for final }
          parts.extend(res.tokens);
          parts.extend(req.insert(") ++", ctx.source_range(0, txt)));
        } else {
          let mut chars = txt.chars();
          match chars.next() {
            None => return Err(NoStringEnd.pack(ctx.source_range(req.tail().len(), ""))),
            Some('\\') => match chars.next() {
              None => write!(str, "\\").expect("writing \\ into string"),
              Some(next) => write!(str, "\\{next}").expect("writing \\ and char into string"),
            },
            Some(c) => write!(str, "{c}").expect("writing char into string"),
          }
          txt = chars.as_str();
        }
      }
    })
  }
}

/// An interpolated string section started with ${ wasn't closed with a balanced
/// }
pub struct UnclosedInterpolation;
impl ParseErrorKind for UnclosedInterpolation {
  const DESCRIPTION: &'static str = "A ${ block within a $-string wasn't closed";
}

/// String literal never ends
pub(super) struct NoStringEnd;
impl ParseErrorKind for NoStringEnd {
  const DESCRIPTION: &'static str = "A string literal was not closed with `\"`";
}

/// A unicode escape sequence contains something other than a hex digit
pub(super) struct NotHex;
impl ParseErrorKind for NotHex {
  const DESCRIPTION: &'static str = "Expected a hex digit";
}

/// A unicode escape sequence contains a number that isn't a unicode code point.
pub(super) struct BadCodePoint;
impl ParseErrorKind for BadCodePoint {
  const DESCRIPTION: &'static str = "\\uXXXX escape sequence does not describe valid code point";
}

/// An unrecognized escape sequence occurred in a string.
pub(super) struct BadEscapeSequence;
impl ParseErrorKind for BadEscapeSequence {
  const DESCRIPTION: &'static str = "Unrecognized escape sequence";
}
#[cfg(test)]
mod test {
  use intern_all::i;

  use super::StringLexer;
  use crate::foreign::atom::Atomic;
  use crate::foreign::inert::Inert;
  use crate::libs::std::string::OrcString;
  use crate::parse::context::MockContext;
  use crate::parse::lex_plugin::{LexPlugReqImpl, LexerPlugin};
  use crate::parse::lexer::Lexeme;
  use crate::parse::parsed::PType;

  #[test]
  fn plain_string() {
    let source = r#""Hello world!" - says the programmer"#;
    let ctx = MockContext::new();
    let req = LexPlugReqImpl { ctx: &ctx, tail: source };
    let res = (StringLexer.lex(&req))
      .expect("the snippet starts with a quote")
      .expect("it contains a valid string");
    let expected = [Inert(OrcString::from("Hello world!")).lexeme()];
    assert_eq!(res.tokens, expected);
    assert_eq!(res.tail, " - says the programmer");
    assert!(!ctx.0.failing(), "No errors were generated")
  }

  #[test]
  #[rustfmt::skip]
  fn template_string() {
    let source = r#""I <${1 + 2} parsers" - this dev"#;
    let ctx = MockContext::new();
    let req = LexPlugReqImpl { ctx: &ctx, tail: source };
    let res = (StringLexer.lex(&req))
      .expect("the snippet starts with a quote")
      .expect("it contains a valid string");
    use Lexeme::{Name, LP, NS, RP};
    let expected = [
      LP(PType::Par),
      Inert(OrcString::from("I <")).lexeme(),
      Name(i!(str: "++")),
      // std::string::convert
      Name(i!(str: "std")), NS, Name(i!(str: "string")), NS, Name(i!(str: "convert")),
      // (1 + 1)
      LP(PType::Par), Inert(1).lexeme(), Name(i!(str: "+")), Inert(2).lexeme(), RP(PType::Par),
      Name(i!(str: "++")),
      Inert(OrcString::from(" parsers")).lexeme(),
      RP(PType::Par),
    ];
    assert_eq!(res.tokens, expected);
    assert_eq!(res.tail, " - this dev");
    assert!(!ctx.0.failing(), "No errors were generated");
  }
}
