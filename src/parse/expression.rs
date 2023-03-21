use std::rc::Rc;

use chumsky::{self, prelude::*, Parser};
use lasso::Spur;
use crate::enum_parser;
use crate::representations::Primitive;
use crate::representations::{Literal, ast::{Clause, Expr}};

use super::lexer::Lexeme;

/// Parses any number of expr wrapped in (), [] or {}
fn sexpr_parser<P>(
  expr: P
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone
where P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone {
  Lexeme::paren_parser(expr.repeated())
    .map(|(del, b)| Clause::S(del, Rc::new(b)))
}

/// Parses `\name.body` or `\name:type.body` where name is any valid name
/// and type and body are both expressions. Comments are allowed
/// and ignored everywhere in between the tokens
fn lambda_parser<'a, P, F>(
  expr: P, intern: &'a F
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone + 'a
where
  P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone + 'a,
  F: Fn(&str) -> Spur + 'a {
  just(Lexeme::BS)
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .ignore_then(namelike_parser(intern))
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .then(
    just(Lexeme::Type)
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .ignore_then(expr.clone().repeated())
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .or_not().map(Option::unwrap_or_default)
  )
  .then_ignore(just(Lexeme::name(".")))
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .then(expr.repeated().at_least(1))
  .map(|((name, typ), body): ((Clause, Vec<Expr>), Vec<Expr>)| {
    Clause::Lambda(Rc::new(name), Rc::new(typ), Rc::new(body))
  })
}

/// see [lambda_parser] but `@` instead of `\` and the name is optional
fn auto_parser<'a, P, F>(
  expr: P, intern: &'a F
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone + 'a
where
  P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone + 'a,
  F: Fn(&str) -> Spur + 'a {
  just(Lexeme::At)
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .ignore_then(namelike_parser(intern).or_not())
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .then(
    just(Lexeme::Type)
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .ignore_then(expr.clone().repeated())
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .or_not().map(Option::unwrap_or_default)
  )
  .then_ignore(just(Lexeme::name(".")))
  .then_ignore(enum_parser!(Lexeme::Comment).repeated())
  .then(expr.repeated().at_least(1))
  .try_map(|((name, typ), body): ((Option<Clause>, Vec<Expr>), Vec<Expr>), s| {
    if name.is_none() && typ.is_empty() {
      Err(Simple::custom(s, "Auto without name or type has no effect"))
    } else {
      Ok(Clause::Auto(name.map(Rc::new), Rc::new(typ), Rc::new(body)))
    }
  })
}

/// Parses a sequence of names separated by :: <br/>
/// Comments are allowed and ignored in between
pub fn ns_name_parser<'a, F>(intern: &'a F)
-> impl Parser<Lexeme, Vec<Spur>, Error = Simple<Lexeme>> + Clone + 'a
where F: Fn(&str) -> Spur + 'a {
  enum_parser!(Lexeme::Name)
    .map(|s| intern(&s))
    .separated_by(
      enum_parser!(Lexeme::Comment).repeated()
      .then(just(Lexeme::NS))
      .then(enum_parser!(Lexeme::Comment).repeated())
    ).at_least(1)
}

/// Parse any legal argument name starting with a `$`
fn placeholder_parser() -> impl Parser<Lexeme, String, Error = Simple<Lexeme>> + Clone {
  enum_parser!(Lexeme::Name).try_map(|name, span| {
    name.strip_prefix('$').map(&str::to_string)
      .ok_or_else(|| Simple::custom(span, "Not a placeholder"))
  })
}

pub fn namelike_parser<'a, F>(intern: &'a F)
-> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone + 'a
where F: Fn(&str) -> Spur + 'a {
  choice((
    just(Lexeme::name("...")).to(true)
      .or(just(Lexeme::name("..")).to(false))
      .then(placeholder_parser())
      .then(
        just(Lexeme::Type)
        .ignore_then(enum_parser!(Lexeme::Uint))
        .or_not().map(Option::unwrap_or_default)
      )
      .map(|((nonzero, key), prio)| Clause::Placeh{key, vec: Some((
        prio.try_into().unwrap(),
        nonzero
      ))}),
    ns_name_parser(intern)
      .map(|qualified| Clause::Name(Rc::new(qualified))),
  ))
}

pub fn clause_parser<'a, P, F>(
  expr: P, intern: &'a F
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone + 'a
where
  P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone + 'a,
  F: Fn(&str) -> Spur + 'a {
  enum_parser!(Lexeme::Comment).repeated()
  .ignore_then(choice((
    enum_parser!(Lexeme >> Literal; Uint, Num, Char, Str)
      .map(Primitive::Literal).map(Clause::P),
    placeholder_parser().map(|key| Clause::Placeh{key, vec: None}),
    namelike_parser(intern),
    sexpr_parser(expr.clone()),
    lambda_parser(expr.clone(), intern),
    auto_parser(expr.clone(), intern),
    just(Lexeme::At).ignore_then(expr.clone()).map(|arg| {
      Clause::Explicit(Rc::new(arg))
    })
  ))).then_ignore(enum_parser!(Lexeme::Comment).repeated())
}

/// Parse an expression
pub fn xpr_parser<'a, F>(intern: &'a F)
-> impl Parser<Lexeme, Expr, Error = Simple<Lexeme>> + 'a
where F: Fn(&str) -> Spur + 'a {
  recursive(|expr| {
    let clause = clause_parser(expr, intern);
    clause.clone().then(
      just(Lexeme::Type)
      .ignore_then(clause.clone())
      .repeated()
    )
    .map(|(val, typ)| Expr(val, Rc::new(typ)))
  }).labelled("Expression")
}
