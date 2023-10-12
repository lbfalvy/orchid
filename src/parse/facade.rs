use super::context::Context;
use super::lexer::lex;
use super::sourcefile::parse_module_body;
use super::stream::Stream;
use crate::error::{ParseErrorWithTokens, ProjectError, ProjectResult};
use crate::representations::sourcefile::FileEntry;

pub fn parse2(ctx: impl Context) -> ProjectResult<Vec<FileEntry>> {
  let tokens = lex(vec![], ctx.source().as_str(), &ctx).expect("debug");
  if tokens.is_empty() {
    Ok(Vec::new())
  } else {
    parse_module_body(Stream::from_slice(&tokens), &ctx).map_err(|error| {
      ParseErrorWithTokens {
        error,
        full_source: ctx.source().to_string(),
        tokens,
      }
      .rc()
    })
  }
}
