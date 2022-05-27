use std::{fmt::Debug};
use chumsky::{self, prelude::*, Parser};

use super::string;
use super::number;
use super::misc;
use super::name;

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Int(u64),
    Char(char),
    Str(String),
    Name(String),
    S(Vec<Expr>),
    Lambda(String, Option<Box<Expr>>, Vec<Expr>),
    Auto(Option<String>, Option<Box<Expr>>, Vec<Expr>),
    Typed(Box<Expr>, Box<Expr>)
}

fn typed_parser<'a>(
    expr: Recursive<'a, char, Expr, Simple<char>>,
    ops: &'a [String]
) -> impl Parser<char, Expr, Error = Simple<char>> + 'a {
    just(':').ignore_then(expr)
}

fn untyped_xpr_parser<'a>(
    expr: Recursive<'a, char, Expr, Simple<char>>,
    ops: &'a [String]
) -> impl Parser<char, Expr, Error = Simple<char>> + 'a {
    let lambda = just('\\')
        .ignore_then(name::name_parser(ops))
        .then(typed_parser(expr.clone(), ops).or_not())
        .then_ignore(just('.'))
        .then(expr.clone().repeated().at_least(1))
        .map(|((name, t), body)| Expr::Lambda(name, t.map(Box::new), body));
    let auto = just('@')
        .ignore_then(name::name_parser(ops).or_not())
        .then(typed_parser(expr.clone(), ops).or_not())
        .then_ignore(just('.'))
        .then(expr.clone().repeated().at_least(1))
        .map(|((name, t), body)| Expr::Auto(name, t.map(Box::new), body));
    let sexpr = expr.clone()
        .repeated()
        .delimited_by(just('('), just(')'))
        .map(Expr::S);
    choice((
        number::float_parser().map(Expr::Num),
        number::int_parser().map(Expr::Int),
        string::char_parser().map(Expr::Char),
        string::str_parser().map(Expr::Str),
        name::name_parser(ops).map(Expr::Name),
        sexpr,
        lambda,
        auto
    )).padded()
}

pub fn expression_parser(ops: &[String]) -> impl Parser<char, Expr, Error = Simple<char>> + '_ {
    return recursive(|expr| {
        return misc::comment_parser().or_not().ignore_then(
            untyped_xpr_parser(expr.clone(), &ops)
                .then(typed_parser(expr, ops).or_not())
                .map(|(val, t)| match t {
                    Some(typ) => Expr::Typed(Box::new(val), Box::new(typ)),
                    None => val
                })
        ).then_ignore(misc::comment_parser().or_not())
    })
}