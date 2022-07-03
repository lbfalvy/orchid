use chumsky::{self, prelude::*, Parser};

use super::{expression, number::float_parser};

#[derive(Debug, Clone)]
pub struct Rule {
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
pub fn rule_parser<'a, T: 'a + AsRef<str> + Clone>(
    pattern_ops: &[T],
    ops: &[T]
) -> impl Parser<char, Rule, Error = Simple<char>> + 'a {
    expression::expression_parser(pattern_ops).padded()
        .then_ignore(just('='))
        .then(
            float_parser().then_ignore(just("=>"))
            .or_not().map(|prio| prio.unwrap_or(0.0))
        ).then(expression::expression_parser(ops).padded())
        .map(|((source, priority), target)| Rule { source, priority, target })
        .labelled("rule")
}
