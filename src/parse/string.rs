//! Parser for a string literal

use intern_all::i;
use itertools::Itertools;

use super::context::ParseCtx;
use super::errors::{BadCodePoint, BadEscapeSequence, NoStringEnd, NotHex, ParseErrorKind};
#[allow(unused)] // for doc
use super::lex_plugin::LexerPlugin;
use super::lexer::{Entry, LexRes, Lexeme};
use crate::error::{ProjectErrorObj, ProjectResult};
use crate::foreign::atom::AtomGenerator;
use crate::foreign::inert::Inert;
use crate::libs::std::string::OrcString;

/// Reasons why [parse_string] might fail. See [StringError]
pub enum StringErrorKind {
  /// A unicode escape sequence wasn't followed by 4 hex digits
  NotHex,
  /// A unicode escape sequence contained an unassigned code point
  BadCodePoint,
  /// An unrecognized escape sequence was found
  BadEscSeq,
}

/// Error produced by [parse_string]
pub struct StringError {
  /// Character where the error occured
  pos: usize,
  /// Reason for the error
  kind: StringErrorKind,
}

impl StringError {
  /// Convert into project error for reporting
  pub fn to_proj(self, ctx: &dyn ParseCtx, pos: usize) -> ProjectErrorObj {
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
pub fn parse_string(str: &str) -> Result<String, StringError> {
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

/// [LexerPlugin] for a string literal.
pub struct StringLexer;
impl LexerPlugin for StringLexer {
  fn lex<'b>(
    &self,
    req: &'_ dyn super::lex_plugin::LexPluginReq<'b>,
  ) -> Option<ProjectResult<super::lexer::LexRes<'b>>> {
    req.tail().strip_prefix('"').map(|data| {
      let mut leftover = data;
      return loop {
        let (inside, outside) = (leftover.split_once('"'))
          .ok_or_else(|| NoStringEnd.pack(req.ctx().source_range(data.len(), "")))?;
        let backslashes = inside.chars().rev().take_while(|c| *c == '\\').count();
        if backslashes % 2 == 0 {
          // cut form tail to recoup what string_content doesn't have
          let (string_data, tail) = data.split_at(data.len() - outside.len() - 1);
          let tail = &tail[1..]; // push the tail past the end quote
          let string =
            parse_string(string_data).map_err(|e| e.to_proj(req.ctx(), req.ctx().pos(data)))?;
          let output = Inert(OrcString::from(i(&string)));
          let ag = AtomGenerator::cloner(output);
          let range = req.ctx().range(string_data.len(), tail);
          let entry = Entry { lexeme: Lexeme::Atom(ag), range };
          break Ok(LexRes { tokens: vec![entry], tail });
        } else {
          leftover = outside;
        }
      };
    })
  }
}

#[cfg(test)]
mod test {
  use super::StringLexer;
  use crate::foreign::inert::Inert;
  use crate::libs::std::string::OrcString;
  use crate::parse::context::MockContext;
  use crate::parse::lex_plugin::{LexPlugReqImpl, LexerPlugin};
  use crate::parse::lexer::{Entry, Lexeme};

  #[test]
  fn plain_string() {
    let source = r#""hello world!" - says the programmer"#;
    let req = LexPlugReqImpl { ctx: &MockContext, tail: source };
    let res = (StringLexer.lex(&req))
      .expect("the snippet starts with a quote")
      .expect("it contains a valid string");
    let ag = match &res.tokens[..] {
      [Entry { lexeme: Lexeme::Atom(ag), .. }] => ag,
      _ => panic!("Expected a single atom"),
    };
    let atom = ag.run().try_downcast::<Inert<OrcString>>().expect("Lexed to inert");
    assert_eq!(atom.0.as_str(), "hello world!");
    assert_eq!(res.tail, " - says the programmer");
  }
}
