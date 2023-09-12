use std::rc::Rc;

use chumsky::Parser;

use super::context::Context;
use super::errors::LexError;
use super::lexer;
use super::sourcefile::parse_module_body;
use super::stream::Stream;
use crate::error::{ParseErrorWithTokens, ProjectError, ProjectResult};
use crate::representations::sourcefile::FileEntry;

pub fn parse2(data: &str, ctx: impl Context) -> ProjectResult<Vec<FileEntry>> {
  let source = Rc::new(data.to_string());
  let lexie = lexer(ctx.clone(), source.clone());
  let tokens = (lexie.parse(data)).map_err(|errors| {
    LexError {
      errors,
      file: ctx.file().as_ref().clone(),
      source: source.clone(),
    }
    .rc()
  })?;
  if tokens.is_empty() {
    Ok(Vec::new())
  } else {
    parse_module_body(Stream::from_slice(&tokens), ctx).map_err(|error| {
      ParseErrorWithTokens { error, full_source: data.to_string(), tokens }.rc()
    })
  }
}
