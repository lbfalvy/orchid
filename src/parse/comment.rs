pub use chumsky::{self, prelude::*, Parser};

/// Parses Lua-style comments
pub fn comment_parser() -> impl Parser<char, String, Error = Simple<char>> {
  choice((
    just("--[").ignore_then(take_until(
      just("]--").ignored()
    )),
    just("--").ignore_then(take_until(
      just("\n").rewind().ignored().or(end())
    ))
  )).map(|(vc, ())| vc).collect().labelled("comment")
}
