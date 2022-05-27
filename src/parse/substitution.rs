use chumsky::{self, prelude::*, Parser};

use super::{expression, number::float_parser};

pub struct Substitution {
    source: expression::Expr,
    priority: f64,
    target: expression::Expr
}

pub fn substitutionParser<'a>(
    ops: &'a [String]
) -> impl Parser<char, Substitution, Error = Simple<char>> + 'a {
    expression::expression_parser(ops)
        .then_ignore(just('='))
        .then(
            float_parser().then_ignore(just("=>"))
            .or_not().map(|prio| prio.unwrap_or(0.0))
        ).then(expression::expression_parser(ops))
        .map(|((source, priority), target)| Substitution { source, priority, target })
}
