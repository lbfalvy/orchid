use std::iter;
use std::rc::Rc;

use crate::representations::sourcefile::FileEntry;
use crate::enum_parser;
use crate::ast::{Expr, Rule};

use super::expression::{xpr_parser, ns_name_parser};
use super::import::import_parser;
use super::lexer::Lexeme;
use chumsky::{Parser, prelude::*};
use lasso::Spur;
use ordered_float::NotNan;

fn rule_parser<'a, F>(intern: &'a F) -> impl Parser<Lexeme, (
  Vec<Expr>, NotNan<f64>, Vec<Expr>
), Error = Simple<Lexeme>> + 'a
where F: Fn(&str) -> Spur + 'a {
  xpr_parser(intern).repeated()
    .then(enum_parser!(Lexeme::Rule))
    .then(xpr_parser(intern).repeated())
    .map(|((a, b), c)| (a, b, c))
    .labelled("Rule")
}

pub fn line_parser<'a, F>(intern: &'a F)
-> impl Parser<Lexeme, FileEntry, Error = Simple<Lexeme>> + 'a
where F: Fn(&str) -> Spur + 'a {
  choice((
    // In case the usercode wants to parse doc
    enum_parser!(Lexeme >> FileEntry; Comment),
    just(Lexeme::Import)
      .ignore_then(import_parser(intern).map(FileEntry::Import))
      .then_ignore(enum_parser!(Lexeme::Comment).or_not()),
    just(Lexeme::Export).map_err_with_span(|e, s| {
      println!("{:?} could not yield an export", s); e
    }).ignore_then(
      just(Lexeme::NS).ignore_then(
        ns_name_parser(intern).map(Rc::new)
        .separated_by(just(Lexeme::name(",")))
        .delimited_by(just(Lexeme::LP('(')), just(Lexeme::RP('(')))
      ).map(FileEntry::Export)
      .or(rule_parser(intern).map(|(source, prio, target)| {
        FileEntry::Rule(Rule {
          source: Rc::new(source),
          prio,
          target: Rc::new(target)
        }, true)
      }))
    ),
    // This could match almost anything so it has to go last
    rule_parser(intern).map(|(source, prio, target)| {
      FileEntry::Rule(Rule{
        source: Rc::new(source),
        prio,
        target: Rc::new(target)
      }, false)
    }),
  ))
}

pub fn split_lines(data: &str) -> impl Iterator<Item = &str> {
  let mut source = data.char_indices();
  let mut last_slice = 0;
  iter::from_fn(move || {
    let mut paren_count = 0;
    while let Some((i, c)) = source.next() {
      match c {
        '(' | '{' | '[' => paren_count += 1,
        ')' | '}' | ']'  => paren_count -= 1,
        '\n' if paren_count == 0 => {
          let begin = last_slice;
          last_slice = i;
          return Some(&data[begin..i]);
        },
        _ => (),
      }
    }
    None
  })
}