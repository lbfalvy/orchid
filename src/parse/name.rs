use chumsky::{self, prelude::*, Parser};

/// Matches any one of the passed operators, preferring longer ones
fn op_parser<'a>(ops: &[impl AsRef<str> + Clone])
-> BoxedParser<'a, char, String, Simple<char>>
{
  let mut sorted_ops: Vec<String> = ops.iter()
    .map(|t| t.as_ref().to_string()).collect();
  sorted_ops.sort_by_key(|op| -(op.len() as i64));
  sorted_ops.into_iter()
    .map(|op| just(op).boxed())
    .reduce(|a, b| a.or(b).boxed())
    .unwrap_or_else(|| {
      empty().map(|()| panic!("Empty isn't meant to match")).boxed()
    }).labelled("operator").boxed()
}

/// Characters that cannot be parsed as part of an operator
/// 
/// The initial operator list overrides this.
static NOT_NAME_CHAR: &[char] = &[
  ':', // used for namespacing and type annotations
  '\\', '@', // parametric expression starters
  '"', '\'', // parsed as primitives and therefore would never match
  '(', ')', '[', ']', '{', '}', // must be strictly balanced
  '.', // Argument-body separator in parametrics
  ',', // used in imports
];

/// Matches anything that's allowed as an operator
/// 
/// FIXME: `@name` without a dot should be parsed correctly for overrides.
/// Could be an operator but then parametrics should take precedence,
/// which might break stuff. investigate.
/// 
/// TODO: `'` could work as an operator whenever it isn't closed.
/// It's common im maths so it's worth a try
/// 
/// TODO: `.` could possibly be parsed as an operator in some contexts.
/// This operator is very common in maths so it's worth a try.
/// Investigate.
pub fn modname_parser<'a>()
-> impl Parser<char, String, Error = Simple<char>> + 'a
{
  filter(move |c| !NOT_NAME_CHAR.contains(c) && !c.is_whitespace())
    .repeated().at_least(1)
    .collect()
    .labelled("modname")
}

/// Parse an operator or name. Failing both, parse everything up to
/// the next whitespace or blacklisted character as a new operator.
pub fn name_parser<'a>(ops: &[impl AsRef<str> + Clone])
-> impl Parser<char, String, Error = Simple<char>> + 'a
{
  choice((
    op_parser(ops), // First try to parse a known operator
    text::ident().labelled("plain text"), // Failing that, parse plain text
    modname_parser() // Finally parse everything until tne next forbidden char
  ))
  .labelled("name")
}

/// Decide if a string can be an operator. Operators can include digits
/// and text, just not at the start.
pub fn is_op(s: impl AsRef<str>) -> bool {
  return match s.as_ref().chars().next() {
    Some(x) => !x.is_alphanumeric(), 
    None => false
  }
}
