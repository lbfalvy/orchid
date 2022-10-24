use chumsky::{self, prelude::*, Parser};
use crate::enum_parser;
use crate::representations::{Literal, ast::{Clause, Expr}};
use crate::utils::{to_mrc_slice, one_mrc_slice};

use super::lexer::Lexeme;

/// Parses any number of expr wrapped in (), [] or {}
fn sexpr_parser<P>(
    expr: P
) -> impl Parser<Lexeme, Clause, Error = Simple<Lexeme>> + Clone
where P: Parser<Lexeme, Expr, Error = Simple<Lexeme>> + Clone {
    Lexeme::paren_parser(expr.repeated()).map(|(del, b)| Clause::S(del, to_mrc_slice(b)))
}

/// Parses `\name.body` or `\name:type.body` where name is any valid name and type and body are
/// both expressions. Comments are allowed and ignored everywhere in between the tokens
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
    .map(|((name, typ), body): ((String, Vec<Expr>), Vec<Expr>)| {
        // for ent in &mut body { ent.bind_parameter(&name) };
        Clause::Lambda(name, to_mrc_slice(typ), to_mrc_slice(body))
    })
}

/// see [lambda_parser] but `@` instead of `\` and the name is optional
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
        .or_not().map(Option::unwrap_or_default)
    )
    .then_ignore(just(Lexeme::name(".")))
    .then_ignore(enum_parser!(Lexeme::Comment).repeated())
    .then(expr.repeated().at_least(1))
    .try_map(|((name, typ), body): ((Option<String>, Vec<Expr>), Vec<Expr>), s| {
        if name.is_none() && typ.is_empty() {
            Err(Simple::custom(s, "Auto without name or type has no effect"))
        } else { 
            Ok(Clause::Auto(name, to_mrc_slice(typ), to_mrc_slice(body)))
        }
    })
}

/// Parses a sequence of names separated by :: <br/>
/// Comments are allowed and ignored in between
fn name_parser() -> impl Parser<Lexeme, Vec<String>, Error = Simple<Lexeme>> + Clone {
    enum_parser!(Lexeme::Name).separated_by(
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

/// Parse an expression
pub fn xpr_parser() -> impl Parser<Lexeme, Expr, Error = Simple<Lexeme>> {
    recursive(|expr| {
        let clause = 
        enum_parser!(Lexeme::Comment).repeated()
        .ignore_then(choice((
            enum_parser!(Lexeme >> Literal; Int, Num, Char, Str).map(Clause::Literal),
            placeholder_parser().map(|key| Clause::Placeh{key, vec: None}),
            just(Lexeme::name("...")).to(true)
                .or(just(Lexeme::name("..")).to(false))
                .then(placeholder_parser())
                .then(
                    just(Lexeme::Type)
                    .ignore_then(enum_parser!(Lexeme::Int))
                    .or_not().map(Option::unwrap_or_default)
                )
                .map(|((nonzero, key), prio)| Clause::Placeh{key, vec: Some((
                    prio.try_into().unwrap(),
                    nonzero
                ))}),
            name_parser().map(|qualified| Clause::Name {
                local: if qualified.len() == 1 {Some(qualified[0].clone())} else {None},
                qualified: to_mrc_slice(qualified)
            }),
            sexpr_parser(expr.clone()),
            lambda_parser(expr.clone()),
            auto_parser(expr.clone()),
            just(Lexeme::At).to(Clause::Name {
                local: Some("@".to_string()),
                qualified: one_mrc_slice("@".to_string())
            })
        ))).then_ignore(enum_parser!(Lexeme::Comment).repeated());
        clause.clone().then(
            just(Lexeme::Type)
            .ignore_then(clause.clone())
            .repeated()
        )
        .map(|(val, typ)| Expr(val, to_mrc_slice(typ)))
    }).labelled("Expression")
}
