use std::ops::Range;
use std::rc::Rc;

use chumsky::{self, prelude::*, Parser};

use crate::enum_filter;
use crate::representations::Primitive;
use crate::representations::ast::{Clause, Expr};
use crate::representations::location::Location;
use crate::interner::Token;

use super::context::Context;
use super::lexer::{Lexeme, Entry, filter_map_lex};

/// Parses any number of expr wrapped in (), [] or {}
fn sexpr_parser(
  expr: impl Parser<Entry, Expr, Error = Simple<Entry>> + Clone
) -> impl Parser<Entry, (Clause, Range<usize>), Error = Simple<Entry>> + Clone {
  let body = expr.repeated();
  choice((
    Lexeme::LP('(').parser().then(body.clone())
      .then(Lexeme::RP('(').parser()),
    Lexeme::LP('[').parser().then(body.clone())
      .then(Lexeme::RP('[').parser()),
    Lexeme::LP('{').parser().then(body.clone())
      .then(Lexeme::RP('{').parser()),
  )).map(|((lp, body), rp)| {
    let Entry{lexeme, range: Range{start, ..}} = lp;
    let end = rp.range.end;
    let char = if let Lexeme::LP(c) = lexeme {c}
    else {unreachable!("The parser only matches Lexeme::LP")};
    (Clause::S(char, Rc::new(body)), start..end)
  }).labelled("S-expression")
}

/// Parses `\name.body` or `\name:type.body` where name is any valid name
/// and type and body are both expressions. Comments are allowed
/// and ignored everywhere in between the tokens
fn lambda_parser<'a>(
  expr: impl Parser<Entry, Expr, Error = Simple<Entry>> + Clone + 'a,
  ctx: impl Context + 'a
) -> impl Parser<Entry, (Clause, Range<usize>), Error = Simple<Entry>> + Clone + 'a {
  Lexeme::BS.parser()
  .ignore_then(expr.clone())
  .then_ignore(Lexeme::Name(ctx.interner().i(".")).parser())
  .then(expr.repeated().at_least(1))
  .map_with_span(move |(arg, body), span| {
    (Clause::Lambda(Rc::new(arg), Rc::new(body)), span)
  }).labelled("Lambda")
}

/// Parses a sequence of names separated by :: <br/>
/// Comments and line breaks are allowed and ignored in between
pub fn ns_name_parser<'a>(ctx: impl Context + 'a)
-> impl Parser<Entry, (Token<Vec<Token<String>>>, Range<usize>), Error = Simple<Entry>> + Clone + 'a
{
  filter_map_lex(enum_filter!(Lexeme::Name))
    .separated_by(Lexeme::NS.parser()).at_least(1)
    .map(move |elements| {
      let start = elements.first().expect("can never be empty").1.start;
      let end = elements.last().expect("can never be empty").1.end;
      let tokens = 
        /*ctx.prefix().iter().copied().chain*/(
          elements.iter().map(|(t, _)| *t)
        ).collect::<Vec<_>>();
      (ctx.interner().i(&tokens), start..end)
    }).labelled("Namespaced name")
}

pub fn namelike_parser<'a>(ctx: impl Context + 'a)
-> impl Parser<Entry, (Clause, Range<usize>), Error = Simple<Entry>> + Clone + 'a
{
  choice((
    filter_map_lex(enum_filter!(Lexeme::PH))
      .map(|(ph, range)| (Clause::Placeh(ph), range)),
    ns_name_parser(ctx)
      .map(|(token, range)| (Clause::Name(token), range)),
  ))
}

pub fn clause_parser<'a>(
  expr: impl Parser<Entry, Expr, Error = Simple<Entry>> + Clone + 'a,
  ctx: impl Context + 'a
) -> impl Parser<Entry, (Clause, Range<usize>), Error = Simple<Entry>> + Clone + 'a {
  choice((
    filter_map_lex(enum_filter!(Lexeme >> Primitive; Literal))
      .map(|(p, s)| (Clause::P(p), s)).labelled("Literal"),
    sexpr_parser(expr.clone()),
    lambda_parser(expr.clone(), ctx.clone()),
    namelike_parser(ctx),
  )).labelled("Clause")
}

/// Parse an expression
pub fn xpr_parser<'a>(ctx: impl Context + 'a)
-> impl Parser<Entry, Expr, Error = Simple<Entry>> + 'a
{
  recursive(move |expr| {
    clause_parser(expr, ctx.clone())
    .map(move |(value, range)| {
      Expr{
        value: value.clone(),
        location: Location::Range { file: ctx.file(), range }
      }
    })
  }).labelled("Expression")
}