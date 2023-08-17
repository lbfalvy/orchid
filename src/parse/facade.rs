use chumsky::Parser;

use super::context::Context;
use super::errors::LexError;
use super::lexer;
use super::sourcefile::parse_module_body;
use super::stream::Stream;
use crate::error::{ParseErrorWithTokens, ProjectError, ProjectResult};
use crate::representations::sourcefile::FileEntry;

pub fn parse2(data: &str, ctx: impl Context) -> ProjectResult<Vec<FileEntry>> {
  let lexie = lexer(ctx.clone());
  let tokens = (lexie.parse(data))
    .map_err(|errors| LexError { errors, file: ctx.file() }.rc())?;
  if tokens.is_empty() {
    Ok(Vec::new())
  } else {
    parse_module_body(Stream::from_slice(&tokens), ctx).map_err(|error| {
      ParseErrorWithTokens { error, full_source: data.to_string(), tokens }.rc()
    })
  }
}
