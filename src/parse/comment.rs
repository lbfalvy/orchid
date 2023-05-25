pub use chumsky::prelude::*;
pub use chumsky::{self, Parser};

use super::decls::SimpleParser;

/// Parses Lua-style comments
pub fn comment_parser() -> impl SimpleParser<char, String> {
  choice((
    just("--[").ignore_then(take_until(just("]--").ignored())),
    just("--").ignore_then(take_until(just("\n").rewind().ignored().or(end()))),
  ))
  .map(|(vc, ())| vc)
  .collect()
  .labelled("comment")
}
