use chumsky::{self, prelude::*, Parser};

/// Parses a text character that is not the specified delimiter
fn text_parser(delim: char) -> impl Parser<char, char, Error = Simple<char>> {
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
      .or(just('u').ignore_then(
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
      )),
  );
  filter(move |&c| c != '\\' && c != delim).or(escape)
}

/// Parse a character literal between single quotes
pub fn char_parser() -> impl Parser<char, char, Error = Simple<char>> {
  just('\'').ignore_then(text_parser('\'')).then_ignore(just('\''))
}

/// Parse a string between double quotes
pub fn str_parser() -> impl Parser<char, String, Error = Simple<char>> {
  just('"')
  .ignore_then(
    text_parser('"').map(Some)
    .or(just("\\\n").map(|_| None)) // Newlines preceded by backslashes are ignored.
    .repeated()
  ).then_ignore(just('"'))
  .flatten().collect()
}
