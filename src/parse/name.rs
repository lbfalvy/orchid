use chumsky::{self, prelude::*, Parser};

/// Matches any one of the passed operators, longest-first
fn op_parser<'a, T: AsRef<str> + Clone>(ops: &[T]) -> BoxedParser<'a, char, String, Simple<char>> {
  let mut sorted_ops: Vec<String> = ops.iter().map(|t| t.as_ref().to_string()).collect();
  sorted_ops.sort_by_key(|op| -(op.len() as i64));
  sorted_ops.into_iter()
    .map(|op| just(op).boxed())
    .reduce(|a, b| a.or(b).boxed())
    .unwrap_or_else(|| empty().map(|()| panic!("Empty isn't meant to match")).boxed())
    .labelled("operator").boxed()
}

/// Matches anything that's allowed as an operator
/// 
/// Blacklist rationale:
/// - `:` is used for namespacing and type annotations, both are distinguished from operators
/// - `\` and `@` are parametric expression starters
/// - `"` and `'` are read as primitives and would never match.
/// - `(` and `)` are strictly balanced and this must remain the case for automation and streaming.
/// - `.` is the discriminator for parametrics.
/// - ',' is always a standalone single operator, so it can never be part of a name
/// 
/// FIXME: `@name` without a dot should be parsed correctly for overrides. Could be an operator but
/// then parametrics should take precedence, which might break stuff. investigate.
/// 
/// TODO: `'` could work as an operator whenever it isn't closed. It's common im maths so it's
/// worth a try
/// 
/// TODO: `.` could possibly be parsed as an operator depending on context. This operator is very
/// common in maths so it's worth a try. Investigate.
pub fn modname_parser<'a>() -> impl Parser<char, String, Error = Simple<char>> + 'a {
  let not_name_char: Vec<char> = vec![':', '\\', '@', '"', '\'', '(', ')', '[', ']', '{', '}', ',', '.'];
  filter(move |c| !not_name_char.contains(c) && !c.is_whitespace())
    .repeated().at_least(1)
    .collect()
    .labelled("modname")
}

/// Parse an operator or name. Failing both, parse everything up to the next whitespace or
/// blacklisted character as a new operator.
pub fn name_parser<'a, T: AsRef<str> + Clone>(
  ops: &[T]
) -> impl Parser<char, String, Error = Simple<char>> + 'a {
  choice((
    op_parser(ops), // First try to parse a known operator
    text::ident().labelled("plain text"), // Failing that, parse plain text
    modname_parser() // Finally parse everything until tne next terminal as a new operator
  ))
  .labelled("name")
}

/// Decide if a string can be an operator. Operators can include digits and text, just not at the
/// start.
pub fn is_op<T: AsRef<str>>(s: T) -> bool {
  return match s.as_ref().chars().next() {
    Some(x) => !x.is_alphanumeric(), 
    None => false
  }
}
