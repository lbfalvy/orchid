use chumsky::{self, prelude::*, Parser};

use super::{expression, number::float_parser};

#[derive(Debug, Clone)]
pub struct Substitution {
    pub source: expression::Expr,
    pub priority: f64,
    pub target: expression::Expr
}

/// Parses a substitution rule of the forms
/// 
/// ```orchid
/// main = \x. ...
/// $a + $b = (add $a $b)
/// (foo bar baz) =1.1=> (foo 1 e)
/// reee =2=> shadow_reee
/// shadow_reee =0.9=> reee
/// ```
/// TBD whether this disables reee in the specified range or loops forever
pub fn substitution_parser<'a>(
    pattern_ops: &[&'a str],
    ops: &[&'a str]
) -> impl Parser<char, Substitution, Error = Simple<char>> + 'a {
    expression::expression_parser(pattern_ops)
        .then_ignore(just('='))
        .then(
            float_parser().then_ignore(just("=>"))
            .or_not().map(|prio| prio.unwrap_or(0.0))
        ).then(expression::expression_parser(ops))
        .map(|((source, priority), target)| Substitution { source, priority, target })
}
