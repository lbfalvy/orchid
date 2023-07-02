use chumsky::prelude::*;
use chumsky::{self, Parser};

use super::decls::SimpleParser;

/// Parses a text character that is not the specified delimiter
fn text_parser(delim: char) -> impl SimpleParser<char, char> {
  // Copied directly from Chumsky's JSON example.
  let escape = just('\\').ignore_then(
    just('\\')
      .or(just('/'))
      .or(just('"'))
      .or(just('b').to('\x08'))
      .or(just('f').to('\x0C'))
      .or(just('n').to('\n'))
      .or(just('r').to('\r'))
      .or(just('t').to('\t'))
      .or(
        just('u').ignore_then(
          filter(|c: &char| c.is_ascii_hexdigit())
            .repeated()
            .exactly(4)
            .collect::<String>()
            .validate(|digits, span, emit| {
              char::from_u32(u32::from_str_radix(&digits, 16).unwrap())
                .unwrap_or_else(|| {
                  emit(Simple::custom(span, "invalid unicode character"));
                  '\u{FFFD}' // unicode replacement character
                })
            }),
        ),
      ),
  );
  filter(move |&c| c != '\\' && c != delim).or(escape)
}

/// Parse a string between double quotes
pub fn str_parser() -> impl SimpleParser<char, String> {
  just('"')
    .ignore_then(
      text_parser('"').map(Some)
    .or(just("\\\n").then(just(' ').or(just('\t')).repeated()).map(|_| None)) // Newlines preceded by backslashes are ignored along with all following indentation.
    .repeated(),
    )
    .then_ignore(just('"'))
    .flatten()
    .collect()
}
