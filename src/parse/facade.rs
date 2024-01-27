//! Entrypoints to the parser that combine lexing and parsing

use super::context::{FlatLocContext, ParseCtx};
use super::frag::Frag;
use super::lexer::lex;
use super::sourcefile::parse_module_body;
use crate::error::ProjectResult;
use crate::location::SourceRange;
use crate::parse::parsed::SourceLine;
use crate::parse::sourcefile::{parse_line, split_lines};

/// Parse a file
pub fn parse_file(ctx: &impl ParseCtx) -> ProjectResult<Vec<SourceLine>> {
  let tokens = lex(vec![], ctx.source().as_str(), ctx, |_| false)?.tokens;
  if tokens.is_empty() {
    Ok(Vec::new())
  } else {
    parse_module_body(Frag::from_slice(&tokens), ctx)
  }
}

/// Parse a statically defined line sequence
///
/// # Panics
///
/// On any parse error, which is why it only accepts a string literal
pub fn parse_entries(
  ctx: &dyn ParseCtx,
  text: &'static str,
  range: SourceRange,
) -> Vec<SourceLine> {
  let ctx = FlatLocContext::new(ctx, &range);
  let res = lex(vec![], text, &ctx, |_| false).expect("pre-specified source");
  split_lines(Frag::from_slice(&res.tokens), &ctx)
    .flat_map(|tokens| parse_line(tokens, &ctx).expect("pre-specified source"))
    .map(|kind| kind.wrap(range.clone()))
    .collect()
}
