use std::iter;
use std::rc::Rc;

use chumsky::prelude::*;
use chumsky::Parser;
use itertools::Itertools;

use super::context::Context;
use super::decls::{SimpleParser, SimpleRecursive};
use super::enum_filter::enum_filter;
use super::expression::xpr_parser;
use super::import::import_parser;
use super::lexer::{filter_map_lex, Lexeme};
use super::Entry;
use crate::ast::{Clause, Constant, Expr, Rule};
use crate::representations::location::Location;
use crate::representations::sourcefile::{FileEntry, Member, Namespace};
use crate::representations::VName;

fn rule_parser<'a>(
  ctx: impl Context + 'a,
) -> impl SimpleParser<Entry, Rule<VName>> + 'a {
  xpr_parser(ctx.clone())
    .repeated()
    .at_least(1)
    .then(filter_map_lex(enum_filter!(Lexeme::Rule)))
    .then(xpr_parser(ctx).repeated().at_least(1))
    .map(|((p, (prio, _)), t)| Rule { pattern: p, prio, template: t })
    .labelled("Rule")
}

fn const_parser<'a>(
  ctx: impl Context + 'a,
) -> impl SimpleParser<Entry, Constant> + 'a {
  filter_map_lex(enum_filter!(Lexeme::Name))
    .then_ignore(Lexeme::Const.parser())
    .then(xpr_parser(ctx.clone()).repeated().at_least(1))
    .map(move |((name, _), value)| Constant {
      name,
      value: if let Ok(ex) = value.iter().exactly_one() {
        ex.clone()
      } else {
        let start = value
          .first()
          .expect("value cannot be empty")
          .location
          .range()
          .expect("all locations in parsed source are known")
          .start;
        let end = value
          .last()
          .expect("asserted right above")
          .location
          .range()
          .expect("all locations in parsed source are known")
          .end;
        Expr {
          location: Location::Range { file: ctx.file(), range: start..end },
          value: Clause::S('(', Rc::new(value)),
        }
      },
    })
}

pub fn collect_errors<T, E: chumsky::Error<T>>(e: Vec<E>) -> E {
  e.into_iter()
    .reduce(chumsky::Error::merge)
    .expect("Error list must be non_enmpty")
}

fn namespace_parser<'a>(
  line: impl SimpleParser<Entry, FileEntry> + 'a,
) -> impl SimpleParser<Entry, Namespace> + 'a {
  Lexeme::Namespace
    .parser()
    .ignore_then(filter_map_lex(enum_filter!(Lexeme::Name)))
    .then(
      any()
        .repeated()
        .delimited_by(Lexeme::LP('(').parser(), Lexeme::RP('(').parser())
        .try_map(move |body, _| {
          split_lines(&body)
            .map(|l| line.parse(l))
            .collect::<Result<Vec<_>, _>>()
            .map_err(collect_errors)
        }),
    )
    .map(move |((name, _), body)| Namespace { name, body })
}

fn member_parser<'a>(
  line: impl SimpleParser<Entry, FileEntry> + 'a,
  ctx: impl Context + 'a,
) -> impl SimpleParser<Entry, Member> + 'a {
  choice((
    namespace_parser(line).map(Member::Namespace),
    rule_parser(ctx.clone()).map(Member::Rule),
    const_parser(ctx).map(Member::Constant),
  ))
}

pub fn line_parser<'a>(
  ctx: impl Context + 'a,
) -> impl SimpleParser<Entry, FileEntry> + 'a {
  recursive(|line: SimpleRecursive<Entry, FileEntry>| {
    choice((
      // In case the usercode wants to parse doc
      filter_map_lex(enum_filter!(Lexeme >> FileEntry; Comment))
        .map(|(ent, _)| ent),
      // plain old imports
      Lexeme::Import
        .parser()
        .ignore_then(import_parser(ctx.clone()).map(FileEntry::Import)),
      Lexeme::Export.parser().ignore_then(choice((
        // token collection
        Lexeme::NS
          .parser()
          .ignore_then(
            filter_map_lex(enum_filter!(Lexeme::Name))
              .map(|(e, _)| e)
              .separated_by(Lexeme::Name(ctx.interner().i(",")).parser())
              .delimited_by(Lexeme::LP('(').parser(), Lexeme::RP('(').parser()),
          )
          .map(FileEntry::Export),
        // public declaration
        member_parser(line.clone(), ctx.clone()).map(FileEntry::Exported),
      ))),
      // This could match almost anything so it has to go last
      member_parser(line, ctx).map(FileEntry::Internal),
    ))
  })
}

pub fn split_lines(data: &[Entry]) -> impl Iterator<Item = &[Entry]> {
  let mut source = data.iter().enumerate();
  let mut last_slice = 0;
  let mut finished = false;
  iter::from_fn(move || {
    let mut paren_count = 0;
    for (i, Entry { lexeme, .. }) in source.by_ref() {
      match lexeme {
        Lexeme::LP(_) => paren_count += 1,
        Lexeme::RP(_) => paren_count -= 1,
        Lexeme::BR if paren_count == 0 => {
          let begin = last_slice;
          last_slice = i + 1;
          return Some(&data[begin..i]);
        },
        _ => (),
      }
    }
    // Include last line even without trailing newline
    if !finished {
      finished = true;
      return Some(&data[last_slice..]);
    }
    None
  })
  .filter(|s| !s.is_empty())
}
