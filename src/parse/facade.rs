use super::context::{Context, FlatLocContext};
use super::lexer::lex;
use super::sourcefile::{parse_exprv, parse_module_body, vec_to_single};
use super::stream::Stream;
use super::{parse_line, split_lines};
use crate::ast::Expr;
use crate::error::ProjectResult;
use crate::representations::sourcefile::FileEntry;
use crate::{Location, VName};

/// Parse a file
pub fn parse_file(ctx: impl Context) -> ProjectResult<Vec<FileEntry>> {
  let tokens = lex(vec![], ctx.source().as_str(), &ctx)?;
  if tokens.is_empty() {
    Ok(Vec::new())
  } else {
    parse_module_body(Stream::from_slice(&tokens), &ctx)
  }
}

/// Parse a ready-made expression
pub fn parse_expr(
  ctx: &impl Context,
  text: &'static str,
  location: Location,
) -> ProjectResult<Expr<VName>> {
  let ctx = FlatLocContext::new(ctx, &location);
  let tokens = lex(vec![], text, &ctx)?;
  let items = parse_exprv(Stream::from_slice(&tokens), None, &ctx)?.0;
  vec_to_single(tokens.first().expect("source must not be empty"), items)
}

/// Parse a ready-made line
pub fn parse_entries(
  ctx: &(impl Context + ?Sized),
  text: &'static str,
  location: Location,
) -> ProjectResult<Vec<FileEntry>> {
  let ctx = FlatLocContext::new(ctx, &location);
  let tokens = lex(vec![], text, &ctx)?;
  let entries = split_lines(Stream::from_slice(&tokens))
    .flat_map(|tokens| parse_line(tokens, &ctx).expect("pre-specified source"))
    .map(|kind| kind.wrap(location.clone()))
    .collect();
  Ok(entries)
}
