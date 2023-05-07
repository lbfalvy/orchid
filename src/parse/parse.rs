use std::fmt::Debug;

use chumsky::{prelude::*, Parser};
use thiserror::Error;

use crate::representations::sourcefile::{FileEntry};
use crate::parse::sourcefile::split_lines;

use super::context::Context;
use super::{lexer, line_parser, Entry};


#[derive(Error, Debug, Clone)]
pub enum ParseError {
  #[error("Could not tokenize {0:?}")]
  Lex(Vec<Simple<char>>),
  #[error("Could not parse {:?} on line {}", .0.first().unwrap().1.span(), .0.first().unwrap().0)]
  Ast(Vec<(usize, Simple<Entry>)>)
}

/// All the data required for parsing


/// Parse a string of code into a collection of module elements;
/// imports, exports, comments, declarations, etc.
/// 
/// Notice that because the lexer splits operators based on the provided
/// list, the output will only be correct if operator list already
/// contains all operators defined or imported by this module.
pub fn parse<'a>(data: &str, ctx: impl Context)
-> Result<Vec<FileEntry>, ParseError>
{
  // TODO: wrap `i`, `ops` and `prefix` in a parsing context
  let lexie = lexer(ctx.clone());
  let token_batchv = lexie.parse(data).map_err(ParseError::Lex)?;
  // println!("Lexed:\n{}", LexedText(token_batchv.clone()).bundle(ctx.interner()));
  // println!("Lexed:\n{:?}", token_batchv.clone());
  let parsr = line_parser(ctx).then_ignore(end());
  let (parsed_lines, errors_per_line) = split_lines(&token_batchv)
    .enumerate()
    .map(|(i, entv)| (i,
      entv.iter()
        .filter(|e| !e.is_filler())
        .cloned()
        .collect::<Vec<_>>()
    ))
    .filter(|(_, l)| l.len() > 0)
    .map(|(i, l)| (i, parsr.parse(l)))
    .map(|(i, res)| match res {
      Ok(r) => (Some(r), (i, vec![])),
      Err(e) => (None, (i, e))
    }).unzip::<_, _, Vec<_>, Vec<_>>();
  let total_err = errors_per_line.into_iter()
    .flat_map(|(i, v)| v.into_iter().map(move |e| (i, e)))
    .collect::<Vec<_>>();
  if !total_err.is_empty() { Err(ParseError::Ast(total_err)) }
  else { Ok(parsed_lines.into_iter().map(Option::unwrap).collect()) } 
}
