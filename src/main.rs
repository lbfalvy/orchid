use std::io::{self, Read};

use chumsky::{Parser, prelude::*};

mod parse;
mod project;
mod utils;

fn main() {
    let mut input = String::new();
    let mut stdin = io::stdin();
    stdin.read_to_string(&mut input).unwrap();
    let ops: Vec<&str> = vec!["$", "."];
    let output = parse::expression_parser(&ops).then_ignore(end()).parse(input);
    println!("\nParsed:\n{:?}", output);
}
