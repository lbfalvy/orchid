use chumsky::{self, prelude::*, Parser};
use crate::{enum_parser, expression::{Clause, Expr, Literal}};

use super::{lexer::Lexeme};

fn sexpr_parser<P>(
    expr: P
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone
where P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone {
    Lexeme::paren_parser(expr.repeated()).map(|(del, b)| Clause::S(del, b))
}

fn lambda_parser<P>(
    expr: P
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone
where P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone {
    just(Lexeme::BS)
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .ignore_then(enum_parser!(Lexeme::Name))
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
    .map(|((name, typ), mut body): ((String, Vec<Expr>), Vec<Expr>)| {
        for ent in &mut body { ent.bind_parameter(&name) };
        Clause::Lambda(name, typ, body)
    })
}

fn auto_parser<P>(
    expr: P
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone
where P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone {
    just(Lexeme::At)
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .ignore_then(enum_parser!(Lexeme::Name).or_not())
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .then(
        just(Lexeme::Type)
        .then_ignore(enum_parser!(Lexeme::Comment).repeated())
        .ignore_then(expr.clone().repeated())
        .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    )
    .then_ignore(just(Lexeme::name(".")))
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .then(expr.repeated().at_least(1))
    .try_map(|((name, typ), mut body), s| if name == None && typ.is_empty() {
        Err(Simple::custom(s, "Auto without name or type has no effect"))
    } else { 
        if let Some(n) = &name {
            for ent in &mut body { ent.bind_parameter(n) }
        }
        Ok(Clause::Auto(name, typ, body))
    })
}

fn name_parser() -> impl Parser<Lexeme, Vec<String>, Error = Simple<Lexeme>> + Clone {
    enum_parser!(Lexeme::Name).separated_by(
        enum_parser!(Lexeme::Comment).repeated()
        .then(just(Lexeme::NS))
        .then(enum_parser!(Lexeme::Comment).repeated())
    ).at_least(1)
}

/// Parse an expression without a type annotation
pub fn xpr_parser() -> impl Parser<Lexeme, Expr, Error = Simple<Lexeme>> {
    recursive(|expr| {
        let clause = 
        enum_parser!(Lexeme::Comment).repeated()
        .ignore_then(choice((
            enum_parser!(Lexeme >> Literal; Int, Num, Char, Str).map(Clause::Literal),
            name_parser().map(Clause::Name),
            sexpr_parser(expr.clone()),
            lambda_parser(expr.clone()),
            auto_parser(expr.clone())
        ))).then_ignore(enum_parser!(Lexeme::Comment).repeated());
        clause.clone().then(
            just(Lexeme::Type)
            .ignore_then(expr.clone()).or_not()
        )
        .map(|(val, typ)| Expr(val, typ.map(Box::new)))
    }).labelled("Expression")
}