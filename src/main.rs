use std::io::{self, Read};

use chumsky::Parser;

mod parse;

fn main() {
    let mut input = String::new();
    let mut stdin = io::stdin();
    stdin.read_to_string(&mut input).unwrap();
    let output = parse::parser().parse(input);
    println!("\nParsed:\n{:?}", output);
}
