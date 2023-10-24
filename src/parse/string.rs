use itertools::Itertools;

use super::context::Context;
#[allow(unused)] // for doc
use super::context::LexerPlugin;
use super::errors::{BadCodePoint, BadEscapeSequence, NoStringEnd, NotHex};
use crate::error::{ProjectError, ProjectResult};
use crate::foreign::Atom;
use crate::OrcString;

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
            let (j, c) = (iter.next())
              .ok_or(StringError { pos, kind: StringErrorKind::NotHex })?;
            pos = j;
            let b =
              u32::from_str_radix(&String::from(c), 16).map_err(|_| {
                StringError { pos, kind: StringErrorKind::NotHex }
              })?;
            Ok(16u32.pow(radical) + b)
          })
          .fold_ok(0, u32::wrapping_add)?;
        char::from_u32(acc)
          .ok_or(StringError { pos, kind: StringErrorKind::BadCodePoint })?
      },
      _ => return Err(StringError { pos, kind: StringErrorKind::BadEscSeq }),
    };
    target.push(next);
  }
  Ok(target)
}

/// [LexerPlugin] for a string literal.
pub fn lex_string<'a>(
  data: &'a str,
  ctx: &dyn Context,
) -> Option<ProjectResult<(Atom, &'a str)>> {
  data.strip_prefix('"').map(|data| {
    let mut leftover = data;
    return loop {
      let (inside, outside) = (leftover.split_once('"'))
        .ok_or_else(|| NoStringEnd(ctx.location(data.len(), "")).rc())?;
      let backslashes = inside.chars().rev().take_while(|c| *c == '\\').count();
      if backslashes % 2 == 0 {
        // cut form tail to recoup what string_content doesn't have
        let (string_data, tail) = data.split_at(data.len() - outside.len() - 1);
        let tail = &tail[1..]; // push the tail past the end quote
        let string = parse_string(string_data).map_err(|e| {
          let start = ctx.pos(data) + e.pos;
          let location = ctx.range_loc(start..start + 1);
          match e.kind {
            StringErrorKind::NotHex => NotHex(location).rc(),
            StringErrorKind::BadCodePoint => BadCodePoint(location).rc(),
            StringErrorKind::BadEscSeq => BadEscapeSequence(location).rc(),
          }
        })?;
        let tok = ctx.interner().i(&string);
        break Ok((Atom::new(OrcString::from(tok)), tail));
      } else {
        leftover = outside;
      }
    };
  })
}
// TODO: rewrite the tree building pipeline step to load files

#[cfg(test)]
mod test {
  use super::lex_string;
  use crate::parse::context::MockContext;
  use crate::{Interner, OrcString};

  #[test]
  fn plain_string() {
    let source = r#""hello world!" - says the programmer"#;
    let i = Interner::new();
    let (data, tail) = lex_string(source, &MockContext(&i))
      .expect("the snippet starts with a quote")
      .expect("it contains a valid string");
    assert_eq!(
      data.try_downcast::<OrcString>().unwrap().as_str(),
      "hello world!"
    );
    assert_eq!(tail, " - says the programmer");
  }
}
