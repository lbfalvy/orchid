use itertools::Itertools;
use orchid_base::interner::intern;
use orchid_base::location::Pos;
use orchid_base::name::VName;
use orchid_base::vname;
use orchid_extension::atom::AtomicFeatures;
use orchid_extension::error::{ErrorSansOrigin, ProjectErrorObj, ProjectResult};
use orchid_extension::lexer::{LexContext, Lexer};
use orchid_extension::tree::{wrap_tokv, OwnedTok, OwnedTokTree};

use super::str_atom::StringAtom;

/// Reasons why [parse_string] might fail. See [StringError]
#[derive(Clone)]
enum StringErrorKind {
  /// A unicode escape sequence wasn't followed by 4 hex digits
  NotHex,
  /// A unicode escape sequence contained an unassigned code point
  BadCodePoint,
  /// An unrecognized escape sequence was found
  BadEscSeq,
}

/// Error produced by [parse_string]
#[derive(Clone)]
struct StringError {
  /// Character where the error occured
  pos: u32,
  /// Reason for the error
  kind: StringErrorKind,
}

#[derive(Clone)]
struct NotHex;
impl ErrorSansOrigin for NotHex {
  const DESCRIPTION: &'static str = "Expected a hex digit";
}

#[derive(Clone)]
struct BadCodePoint;
impl ErrorSansOrigin for BadCodePoint {
  const DESCRIPTION: &'static str = "The specified number is not a Unicode code point";
}

#[derive(Clone)]
struct BadEscapeSequence;
impl ErrorSansOrigin for BadEscapeSequence {
  const DESCRIPTION: &'static str = "Unrecognized escape sequence";
}

impl StringError {
  /// Convert into project error for reporting
  pub fn into_proj(self, pos: u32) -> ProjectErrorObj {
    let start = pos + self.pos;
    let pos = Pos::Range(start..start + 1);
    match self.kind {
      StringErrorKind::NotHex => NotHex.bundle(&pos),
      StringErrorKind::BadCodePoint => BadCodePoint.bundle(&pos),
      StringErrorKind::BadEscSeq => BadEscapeSequence.bundle(&pos),
    }
  }
}

/// Process escape sequences in a string literal
fn parse_string(str: &str) -> Result<String, StringError> {
  let mut target = String::new();
  let mut iter = str.char_indices().map(|(i, c)| (i as u32, c));
  while let Some((_, c)) = iter.next() {
    if c != '\\' {
      target.push(c);
      continue;
    }
    let (mut pos, code) = iter.next().expect("lexer would have continued");
    let next = match code {
      c @ ('\\' | '"' | '$') => c,
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

#[derive(Clone)]
pub struct NoStringEnd;
impl ErrorSansOrigin for NoStringEnd {
  const DESCRIPTION: &'static str = "String never terminated with \"";
}

#[derive(Default)]
pub struct StringLexer;
impl Lexer for StringLexer {
  const CHAR_FILTER: &'static [std::ops::RangeInclusive<char>] = &['"'..='"'];
  fn lex<'a>(
    full_string: &'a str,
    ctx: &'a LexContext<'a>,
  ) -> Option<ProjectResult<(&'a str, OwnedTokTree)>> {
    full_string.strip_prefix('"').map(|mut tail| {
      let mut parts = vec![];
      let mut cur = String::new();
      let commit_str = |str: &mut String, tail: &str, parts: &mut Vec<OwnedTokTree>| {
        let str_val = parse_string(str)
          .inspect_err(|e| ctx.report(e.clone().into_proj(ctx.pos(tail) - str.len() as u32)))
          .unwrap_or_default();
        let tok = OwnedTok::Atom(StringAtom::new_int(intern(&str_val)).factory());
        parts.push(tok.at(ctx.tok_ran(str.len() as u32, tail)));
        *str = String::new();
      };
      loop {
        if let Some(rest) = tail.strip_prefix('"') {
          commit_str(&mut cur, tail, &mut parts);
          return Ok((rest, wrap_tokv(parts, ctx.pos(full_string)..ctx.pos(rest))));
        } else if let Some(rest) = tail.strip_prefix('$') {
          commit_str(&mut cur, tail, &mut parts);
          parts.push(OwnedTok::Name(VName::literal("++")).at(ctx.tok_ran(1, rest)));
          parts.push(OwnedTok::Name(vname!(std::string::convert)).at(ctx.tok_ran(1, rest)));
          match ctx.recurse(rest) {
            Ok((new_tail, tree)) => {
              tail = new_tail;
              parts.push(tree);
            },
            Err(e) => {
              ctx.report(e.clone());
              return Ok(("", wrap_tokv(parts, ctx.pos(full_string)..ctx.pos(rest))));
            },
          }
        } else if tail.starts_with('\\') {
          // parse_string will deal with it, we just have to make sure we skip the next
          // char
          tail = &tail[2..];
        } else {
          let mut ch = tail.chars();
          if let Some(c) = ch.next() {
            cur.push(c);
            tail = ch.as_str();
          } else {
            let range = ctx.pos(full_string)..ctx.pos("");
            commit_str(&mut cur, tail, &mut parts);
            ctx.report(NoStringEnd.bundle(&Pos::Range(range.clone())));
            return Ok(("", wrap_tokv(parts, range)));
          }
        }
      }
    })
  }
}
