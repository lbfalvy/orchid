use chumsky::{self, prelude::*, Parser};

fn op_parser_recur<'a, 'b>(ops: &'a [String]) -> BoxedParser<'b, char, String, Simple<char>> {
    if ops.len() == 1 { just(ops[0].clone()).boxed() }
    else { just(ops[0].clone()).or(op_parser_recur(&ops[1..])).boxed() }
}

fn op_parser(ops: &[String]) -> BoxedParser<char, String, Simple<char>> {
    let mut sorted_ops = ops.to_vec();
    sorted_ops.sort_by(|a, b| b.len().cmp(&a.len()));
    op_parser_recur(&sorted_ops)
}

pub fn modname_parser() -> impl Parser<char, String, Error = Simple<char>> {
    let not_name_char: Vec<char> = vec![':', '\\', '"', '\'', '(', ')', '.'];
    filter(move |c| !not_name_char.contains(c) && !c.is_whitespace())
        .repeated().at_least(1)
        .collect()
}

pub fn name_parser<'a>(ops: &'a [String]) -> impl Parser<char, String, Error = Simple<char>> + 'a {
    choice((
        op_parser(ops), // First try to parse a known operator
        text::ident(), // Failing that, parse plain text
        // Finally parse everything until tne next terminal as a new operator
        modname_parser()
    )).padded()
}