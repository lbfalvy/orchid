use std::{ops::Range, fmt::Debug};

use chumsky::{prelude::{Simple, end}, Stream, Parser};
use itertools::Itertools;
use thiserror::Error;

use crate::{ast::Rule, parse::lexer::LexedText};

use super::{Lexeme, FileEntry, lexer, line_parser, LexerEntry};


#[derive(Error, Debug, Clone)]
pub enum ParseError {
  #[error("Could not tokenize {0:?}")]
  Lex(Vec<Simple<char>>),
  #[error("Could not parse {0:#?}")]
  Ast(Vec<Simple<Lexeme>>)
}

pub fn parse<'a, Iter, S, Op>(ops: &[Op], stream: S) -> Result<Vec<FileEntry>, ParseError>
where
  Op: 'a + AsRef<str> + Clone,
  Iter: Iterator<Item = (char, Range<usize>)> + 'a,
  S: Into<Stream<'a, char, Range<usize>, Iter>> {
  let lexed = lexer(ops).parse(stream).map_err(ParseError::Lex)?;
  println!("Lexed:\n{:?}", lexed);
  let LexedText(token_batchv) = lexed;
  let parsr = line_parser().then_ignore(end());
  let (parsed_lines, errors_per_line) = token_batchv.into_iter().filter(|v| {
    !v.is_empty()
  }).map(|v| {
    // Find the first invalid position for Stream::for_iter
    let LexerEntry(_, Range{ end, .. }) = v.last().unwrap().clone();
    // Stream expects tuples, lexer outputs structs
    let tuples = v.into_iter().map_into::<(Lexeme, Range<usize>)>();
    parsr.parse(Stream::from_iter(end..end+1, tuples))
    //              ^^^^^^^^^^
    // I haven't the foggiest idea why this is needed, parsers are supposed to be lazy so the
    // end of input should make little difference
  }).map(|res| match res {
    Ok(r) => (Some(r), vec![]),
    Err(e) => (None, e)
  }).unzip::<_, _, Vec<_>, Vec<_>>();
  let total_err = errors_per_line.into_iter()
    .flat_map(Vec::into_iter)
    .collect::<Vec<_>>();
  if !total_err.is_empty() { Err(ParseError::Ast(total_err)) }
  else { Ok(parsed_lines.into_iter().map(Option::unwrap).collect()) } 
}

pub fn reparse<'a, Iter, S, Op>(ops: &[Op], stream: S, pre: &[FileEntry])
-> Result<Vec<FileEntry>, ParseError>
where
  Op: 'a + AsRef<str> + Clone,
  Iter: Iterator<Item = (char, Range<usize>)> + 'a,
  S: Into<Stream<'a, char, Range<usize>, Iter>> {
  let result = parse(ops, stream)?;
  Ok(result.into_iter().zip(pre.iter()).map(|(mut output, donor)| {
    if let FileEntry::Rule(Rule{source, ..}, _) = &mut output {
      if let FileEntry::Rule(Rule{source: s2, ..}, _) = donor {
        *source = s2.clone()
      } else {
        panic!("Preparse and reparse received different row types!")
      }
    }
    output
  }).collect())
}
