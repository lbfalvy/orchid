//! Types for interacting with the Orchid parser, and parts of the parser
//! plugins can use to match the language's behaviour on certain tasks
mod context;
pub mod errors;
mod facade;
mod lexer;
mod multiname;
mod numeric;
mod sourcefile;
mod stream;
mod string;

pub use context::{
  Context, LexerPlugin, LexerPluginOut, LineParser, LineParserOut,
  ParsingContext,
};
pub use facade::{parse_entries, parse_expr, parse_file};
pub use lexer::{namechar, namestart, opchar, split_filter, Entry, Lexeme};
pub use multiname::parse_multiname;
pub use numeric::{
  lex_numeric, numchar, numstart, parse_num, print_nat16, NumError,
  NumErrorKind,
};
pub use sourcefile::{
  expr_slice_location, parse_const, parse_exprv, parse_line, parse_module,
  parse_module_body, parse_rule, split_lines, vec_to_single, parse_nsname
};
pub use stream::Stream;
pub use string::{lex_string, parse_string, StringError, StringErrorKind};
