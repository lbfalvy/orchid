use std::collections::HashMap;

use crate::parse;
use super::expr;

/// Replaces the first element of a name with the matching prefix from a prefix map
fn qualify(
    name: &Vec<String>,
    prefixes: &HashMap<String, Vec<String>>
) -> Option<Vec<String>> {
    let value = prefixes.iter().find(|(k, _)| &&name[0] == k)?.1;
    Some(value.iter().chain(name.iter().skip(1)).cloned().collect())
}

/// Produce a Token object for any value of parse::Expr other than Typed.
/// Called by [#prefix] which handles Typed.
fn prefix_token(
    expr: &parse::Expr,
    namespace: &Vec<String>
) -> expr::Token {
    match expr {
        parse::Expr::Typed(_, _) => panic!("This function should only be called by prefix!"),
        parse::Expr::Char(c) => expr::Token::Literal(expr::Literal::Char(*c)),
        parse::Expr::Int(i) => expr::Token::Literal(expr::Literal::Int(*i)),
        parse::Expr::Num(n) => expr::Token::Literal(expr::Literal::Num(*n)),
        parse::Expr::Str(s) => expr::Token::Literal(expr::Literal::Str(s.clone())),
        parse::Expr::S(v) => expr::Token::S(v.iter().map(|e| prefix(e, namespace)).collect()),
        parse::Expr::Auto(name, typ, body) => expr::Token::Auto(
            name.clone(),
            typ.clone().map(|expr| Box::new(prefix(&expr, namespace))),
            body.iter().map(|e| prefix(e, namespace)).collect(),
        ),
        parse::Expr::Lambda(name, typ, body) => expr::Token::Lambda(
            name.clone(),
            typ.clone().map(|expr| Box::new(prefix(&expr, namespace))),
            body.iter().map(|e| prefix(e, namespace)).collect(),
        ),
        parse::Expr::Name(name) => expr::Token::Name {
            qualified: namespace.iter().chain(name.iter()).cloned().collect(),
            local: if name.len() == 1 {
                Some(name[0].clone())
            } else {
                None
            },
        },
    }
}

/// Produce an Expr object for any value of parse::Expr
pub fn prefix(expr: &parse::Expr, namespace: &Vec<String>) -> expr::Expr {
    match expr {
        parse::Expr::Typed(x, t) => expr::Expr {
            typ: Some(Box::new(prefix(t, namespace))),
            token: prefix_token(x, namespace),
        },
        _ => expr::Expr {
            typ: None,
            token: prefix_token(expr, namespace),
        },
    }
}
