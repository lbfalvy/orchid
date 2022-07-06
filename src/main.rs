
use std::env::current_dir;

mod parse;
mod project;
mod utils;
mod expression;
mod rule;
use project::{rule_collector, Loaded, file_loader};

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
    let cwd = current_dir().unwrap();
    let collect_rules = rule_collector(move |n| {
        if n == vec!["prelude"] { Ok(Loaded::Module(PRELUDE.to_string())) }
        else { file_loader(cwd.clone())(n) }
    }, literal(&["...", ">>", ">>=", "[", "]", ",", "$", "=", "=>"]));
    match collect_rules.try_find(&literal(&["main"])) {
        Ok(rules) => for rule in rules.iter() {
            println!("{rule:?}")
        }
        Err(err) => println!("{:#?}", err)
    }
}
