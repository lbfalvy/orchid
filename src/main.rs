use core::ops::Range;
use std::{env::current_dir, fs::read_to_string};
use std::io;

use chumsky::prelude::end;
use chumsky::{Parser, Stream};
use parse::{LexerEntry, FileEntry};
// use project::{rule_collector, file_loader, Loaded};

mod parse;
mod project;
mod utils;
mod expression;
pub use expression::*;

fn literal(orig: &[&str]) -> Vec<String> {
    orig.iter().map(|&s| s.to_owned()).collect()
}

static PRELUDE:&str = r#"
export ... $name =1000=> (match_seqence $name)
export ] =1000=> conslist_carriage(none)
export , $name conslist_carriage($tail) =1000=> conslist_carriage((some (cons $name $tail)))
export [ $name conslist_carriage($tail) =1000=> (some (cons $name $tail))
export (match_sequence $lhs) >> (match_sequence $rhs) =100=> (bind ($lhs) (\_. $rhs))
export (match_sequence $lhs) >>= (match_sequence $rhs) =100=> (bind ($lhs) ($rhs))
"#;

fn main() {
    // let mut input = String::new();
    // let mut stdin = io::stdin();
    // stdin.read_to_string(&mut input).unwrap();
    let ops: Vec<&str> = vec!["...", ">>", ">>=", "[", "]", ",", "$"];
    let data = read_to_string("./main.orc").unwrap();
    let lexed = parse::lexer(&ops).parse(data).unwrap();
    println!("Lexed: {:?}", lexed);
    let parsr = parse::line_parser().then_ignore(end());
    // match parsr.parse(data) {
    //     Ok(output) => println!("\nParsed:\n{:?}", output),
    //     Err(e) => println!("\nErrored:\n{:?}", e)
    // }
    let lines = lexed.iter().filter_map(|v| {
        let parse::LexerEntry(_, Range{ end, .. }) = v.last().unwrap().clone();
        let tuples = v.into_iter().map(|LexerEntry(l, r)| (l.clone(), r.clone()));
        Some(parsr.parse_recovery_verbose(Stream::from_iter(end..end+1, tuples)))
    }).collect::<Vec<_>>();
    for (id, (out, errs)) in lines.into_iter().enumerate() {
        println!("Parsing line {}", id);
        if let Some(output) = out { println!("Parsed:\n{:?}", output) }
        else { println!("Failed to produce output")}
        if errs.len() > 0 { println!("Errored:\n{:?}", errs)}
    }
    // let output = parse::file_parser(&ops, &ops).parse(data).unwrap();
    // let cwd = current_dir().unwrap();
    // let collect_rules = rule_collector(move |n| {
    //     if n == vec!["prelude"] { Ok(Loaded::Module(PRELUDE.to_string())) }
    //     else { file_loader(cwd.clone())(n) }
    // }, literal(&["...", ">>", ">>=", "[", "]", ","]));
    // let rules = collect_rules.try_find(&literal(&["main"])).unwrap();
    // for rule in rules.iter() {
    //     println!("{:?} ={}=> {:?}", rule.source, rule.priority, rule.target)
    // }
}
