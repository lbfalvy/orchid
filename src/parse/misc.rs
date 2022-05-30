pub use chumsky::{self, prelude::*, Parser};

/// Parses Lua-style comments
pub fn comment_parser() -> impl Parser<char, String, Error = Simple<char>> {
    any().repeated().delimited_by(just("--["), just("]--")).or(
        any().repeated().delimited_by(just("--"), just("\n"))
    ).map(|vc| vc.iter().collect()).padded()
}
