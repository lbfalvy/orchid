use chumsky::{self, prelude::*, Parser};

/// Matches any one of the passed operators, longest-first
fn op_parser<'a>(ops: &[&'a str]) -> BoxedParser<'a, char, String, Simple<char>> {
    let mut sorted_ops = ops.to_vec();
    sorted_ops.sort_by(|a, b| b.len().cmp(&a.len()));
    sorted_ops.into_iter()
        .map(|op| just(op.to_string()).boxed())
        .reduce(|a, b| a.or(b).boxed()).unwrap()
}

/// Matches anything that's allowed as an operator
/// 
/// Blacklist rationale:
/// - `:` is used for namespacing and type annotations, both are distinguished from operators
/// - `\` and `@` are parametric expression starters
/// - `"` and `'` are read as primitives and would never match.
/// - `(` and `)` are strictly balanced and this must remain the case for automation and streaming.
/// - `.` is the discriminator for parametrics.
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
    let not_name_char: Vec<char> = vec![':', '\\', '@', '"', '\'', '(', ')', '.'];
    filter(move |c| !not_name_char.contains(c) && !c.is_whitespace())
        .repeated().at_least(1)
        .collect()
}

/// Parse an operator or name. Failing both, parse everything up to the next whitespace or
/// blacklisted character as a new operator.
pub fn name_parser<'a>(
    ops: &[&'a str]
) -> impl Parser<char, Vec<String>, Error = Simple<char>> + 'a {
    choice((
        op_parser(ops), // First try to parse a known operator
        text::ident(), // Failing that, parse plain text
        modname_parser() // Finally parse everything until tne next terminal as a new operator
    )).padded().separated_by(just("::")).padded()
}