use std::{fmt::Debug};
use chumsky::{self, prelude::*, Parser};

use super::string;
use super::number;
use super::misc;
use super::name;

/// An S-expression as read from a source file
#[derive(Debug, Clone)]
pub enum Expr {
    Num(f64),
    Int(u64),
    Char(char),
    Str(String),
    Name(Vec<String>),
    S(Vec<Expr>),
    Lambda(String, Option<Box<Expr>>, Vec<Expr>),
    Auto(Option<String>, Option<Box<Expr>>, Vec<Expr>),
    Typed(Box<Expr>, Box<Expr>)
}

/// Parse a type annotation
fn typed_parser<'a>(
    expr: Recursive<'a, char, Expr, Simple<char>>
) -> impl Parser<char, Expr, Error = Simple<char>> + 'a {
    just(':').ignore_then(expr)
}

/// Parse an expression without a type annotation
fn untyped_xpr_parser<'a>(
    expr: Recursive<'a, char, Expr, Simple<char>>,
    ops: &[&'a str]
) -> impl Parser<char, Expr, Error = Simple<char>> + 'a {
    // basic S-expression rule
    let sexpr = expr.clone()
        .repeated()
        .delimited_by(just('('), just(')'))
        .map(Expr::S);
    // Blocks
    // can and therefore do match everything up to the closing paren
    // \name. body
    // \name:type. body
    let lambda = just('\\')
        .ignore_then(text::ident())
        .then(typed_parser(expr.clone()).or_not())
        .then_ignore(just('.'))
        .then(expr.clone().repeated().at_least(1))
        .map(|((name, t), body)| Expr::Lambda(name, t.map(Box::new), body));
    // @name. body
    // @name:type. body
    // @:type. body
    let auto = just('@')
        .ignore_then(text::ident().or_not())
        .then(typed_parser(expr.clone()).or_not())
        .then_ignore(just('.'))
        .then(expr.clone().repeated().at_least(1))
        .map(|((name, t), body)| Expr::Auto(name, t.map(Box::new), body));
    choice((
        number::int_parser().map(Expr::Int), // all ints are valid floats so it takes precedence
        number::float_parser().map(Expr::Num),
        string::char_parser().map(Expr::Char),
        string::str_parser().map(Expr::Str),
        name::name_parser(ops).map(Expr::Name), // includes namespacing
        sexpr,
        lambda,
        auto
    )).padded()
}

/// Parse any expression with a type annotation, surrounded by comments
pub fn expression_parser<'a>(ops: &[&'a str]) -> impl Parser<char, Expr, Error = Simple<char>> + 'a {
    // This approach to parsing comments is ugly and error-prone,
    // but I don't have a lot of other ideas
    return recursive(|expr| {
        return misc::comment_parser().or_not().ignore_then(
            untyped_xpr_parser(expr.clone(), &ops)
                .then(typed_parser(expr).or_not())
                .map(|(val, t)| match t {
                    Some(typ) => Expr::Typed(Box::new(val), Box::new(typ)),
                    None => val
                })
        ).then_ignore(misc::comment_parser().or_not())
    })
}