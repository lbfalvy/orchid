#![feature(specialization)]

use std::{env::current_dir, process::exit};

mod parse;
mod project;
mod utils;
mod expression;
mod rule;
use expression::{Expr, Clause};
use mappable_rc::Mrc;
use project::{rule_collector, Loaded, file_loader};
use rule::Repository;
use utils::to_mrc_slice;

fn literal(orig: &[&str]) -> Mrc<[String]> {
    to_mrc_slice(vliteral(orig))
}

fn vliteral(orig: &[&str]) -> Vec<String> {
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


fn initial_tree() -> Mrc<[Expr]> {
    to_mrc_slice(vec![Expr(Clause::Name {
        local: None,
        qualified: to_mrc_slice(vec!["main".to_string(), "main".to_string()])
    }, None)])
}

fn main() {
    let cwd = current_dir().unwrap();
    let collect_rules = rule_collector(move |n| {
        if n == literal(&["prelude"]) { Ok(Loaded::Module(PRELUDE.to_string())) }
        else { file_loader(cwd.clone())(n) }
    }, vliteral(&["...", ">>", ">>=", "[", "]", ",", "=", "=>"]));
    match collect_rules.try_find(&literal(&["main"])) {
        Ok(rules) => {
            let mut tree = initial_tree();
            println!("Start processing {tree:?}");
            let repo = Repository::new(rules.as_ref().to_owned());
            println!("Ruleset: {repo:?}");
            let mut i = 0; loop {
                if 10 <= i {break} else {i += 1}
                match repo.step(Mrc::clone(&tree)) {
                    Ok(Some(phase)) => {
                        tree = phase;
                        println!("Step {i}: {tree:?}")
                    },
                    Ok(None) => exit(0),
                    Err(e) => {
                        eprintln!("Rule error: {e:?}");
                        exit(0)
                    }
                }
            }
        }
        Err(err) => println!("{:#?}", err)
    }
}
