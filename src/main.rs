use std::io::{self, Read};

use chumsky::{Parser, prelude::*};

mod parse;

fn main() {
    let mut input = String::new();
    let mut stdin = io::stdin();
    stdin.read_to_string(&mut input).unwrap();
    let ops: Vec<String> = vec!["$", "."].iter().map(|&s| s.to_string()).collect();
    let output = parse::expression_parser(&ops).then_ignore(end()).parse(input);
    println!("\nParsed:\n{:?}", output);
}
