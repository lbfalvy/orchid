use chumsky::prelude::*;
use chumsky::{self, Parser};

use super::decls::{BoxedSimpleParser, SimpleParser};

/// Matches any one of the passed operators, preferring longer ones
fn op_parser<'a>(
  ops: &[impl AsRef<str> + Clone],
) -> BoxedSimpleParser<'a, char, String> {
  let mut sorted_ops: Vec<String> =
    ops.iter().map(|t| t.as_ref().to_string()).collect();
  sorted_ops.sort_by_key(|op| -(op.len() as i64));
  sorted_ops
    .into_iter()
    .map(|op| just(op).boxed())
    .reduce(|a, b| a.or(b).boxed())
    .unwrap_or_else(|| {
      empty().map(|()| panic!("Empty isn't meant to match")).boxed()
    })
    .labelled("operator")
    .boxed()
}

/// Characters that cannot be parsed as part of an operator
///
/// The initial operator list overrides this.
pub static NOT_NAME_CHAR: &[char] = &[
  ':', // used for namespacing and type annotations
  '\\', '@', // parametric expression starters
  '"', // parsed as primitive and therefore would never match
  '(', ')', '[', ']', '{', '}', // must be strictly balanced
  '.', // Argument-body separator in parametrics
  ',', // Import separator
];

/// Matches anything that's allowed as an operator
///
/// FIXME: `@name` without a dot should be parsed correctly for overrides.
/// Could be an operator but then parametrics should take precedence,
/// which might break stuff. investigate.
///
/// TODO: `.` could possibly be parsed as an operator in some contexts.
/// This operator is very common in maths so it's worth a try.
/// Investigate.
pub fn anyop_parser<'a>() -> impl SimpleParser<char, String> + 'a {
  filter(move |c| {
    !NOT_NAME_CHAR.contains(c)
      && !c.is_whitespace()
      && !c.is_alphanumeric()
      && c != &'_'
  })
  .repeated()
  .at_least(1)
  .collect()
  .labelled("anyop")
}

/// Parse an operator or name. Failing both, parse everything up to
/// the next whitespace or blacklisted character as a new operator.
pub fn name_parser<'a>(
  ops: &[impl AsRef<str> + Clone],
) -> impl SimpleParser<char, String> + 'a {
  choice((
    op_parser(ops), // First try to parse a known operator
    text::ident().labelled("plain text"), // Failing that, parse plain text
    anyop_parser(), // Finally parse everything until tne next forbidden char
  ))
  .labelled("name")
}
