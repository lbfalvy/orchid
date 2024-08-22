use itertools::Itertools;
use orchid_base::error::{mk_err, OrcErr, OrcRes};
use orchid_base::interner::intern;
use orchid_base::location::Pos;
use orchid_base::tree::{vname_tv, wrap_tokv};
use orchid_base::{intern, vname};
use orchid_extension::atom::AtomicFeatures;
use orchid_extension::lexer::{err_lexer_na, LexContext, Lexer};
use orchid_extension::tree::{GenTok, GenTokTree};

use super::str_atom::IntStrAtom;

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

impl StringError {
  /// Convert into project error for reporting
  pub fn into_proj(self, pos: u32) -> OrcErr {
    let start = pos + self.pos;
    mk_err(
      intern!(str: "Failed to parse string"),
      match self.kind {
        StringErrorKind::NotHex => "Expected a hex digit",
        StringErrorKind::BadCodePoint => "The specified number is not a Unicode code point",
        StringErrorKind::BadEscSeq => "Unrecognized escape sequence",
      },
      [Pos::Range(start..start + 1).into()],
    )
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

#[derive(Default)]
pub struct StringLexer;
impl Lexer for StringLexer {
  const CHAR_FILTER: &'static [std::ops::RangeInclusive<char>] = &['"'..='"'];
  fn lex<'a>(all: &'a str, ctx: &'a LexContext<'a>) -> OrcRes<(&'a str, GenTokTree<'a>)> {
    let mut tail = all.strip_prefix('"').ok_or_else(err_lexer_na)?;
    let mut parts = Vec::<GenTokTree<'a>>::new();
    let mut cur = String::new();
    let mut errors = vec![];
    let commit_str =
      |str: &mut String, tail: &str, err: &mut Vec<OrcErr>, parts: &mut Vec<GenTokTree<'a>>| {
        let str_val = parse_string(str)
          .inspect_err(|e| err.push(e.clone().into_proj(ctx.pos(tail) - str.len() as u32)))
          .unwrap_or_default();
        let tok = GenTok::X(IntStrAtom::from(intern(&*str_val)).factory());
        parts.push(tok.at(ctx.tok_ran(str.len() as u32, tail)));
        *str = String::new();
      };
    loop {
      if let Some(rest) = tail.strip_prefix('"') {
        commit_str(&mut cur, tail, &mut errors, &mut parts);
        return Ok((rest, wrap_tokv(parts, ctx.pos(all)..ctx.pos(rest))));
      } else if let Some(rest) = tail.strip_prefix('$') {
        commit_str(&mut cur, tail, &mut errors, &mut parts);
        parts.push(GenTok::Name(intern!(str: "++")).at(ctx.tok_ran(1, rest)));
        parts.extend(vname_tv(&vname!(std::string::convert), ctx.tok_ran(1, rest)));
        let (new_tail, tree) = ctx.recurse(rest)?;
        tail = new_tail;
        parts.push(tree);
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
          let range = ctx.pos(all)..ctx.pos("");
          commit_str(&mut cur, tail, &mut errors, &mut parts);
          return Err(vec![mk_err(
            intern!(str: "No string end"),
            "String never terminated with \"",
            [Pos::Range(range.clone()).into()],
          )]);
        }
      }
    }
  }
}