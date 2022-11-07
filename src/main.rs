#![feature(specialization)]
#![feature(core_intrinsics)]

use std::env::current_dir;

mod executor;
mod parse;
mod project;
mod utils;
mod representations;
mod rule;
mod types;
use file_loader::LoadingError;
pub use representations::ast;
use ast::{Expr, Clause};
use representations::typed as t;
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
        qualified: literal(&["main", "main"])
    }, to_mrc_slice(vec![]))])
}

#[allow(unused)]
fn typed_notation_debug() {
    let t = t::Clause::Auto(None,
        t::Clause::Lambda(Some(Mrc::new(t::Clause::Argument(0))), 
            t::Clause::Lambda(Some(Mrc::new(t::Clause::Argument(1))),
                t::Clause::Argument(1).wrap_t(t::Clause::Argument(2))
            ).wrap()
        ).wrap()
    ).wrap();
    let f = t::Clause::Auto(None,
        t::Clause::Lambda(Some(Mrc::new(t::Clause::Argument(0))),
            t::Clause::Lambda(Some(Mrc::new(t::Clause::Argument(1))),
                t::Clause::Argument(0).wrap_t(t::Clause::Argument(2))
            ).wrap()
        ).wrap()
    ).wrap();
    println!("{:?}", t::Clause::Apply(t::Clause::Apply(Mrc::clone(&t), t).wrap(), f))
}

#[allow(unused)]
fn load_project() {
    let cwd = current_dir().unwrap();
    let collect_rules = rule_collector(move |n| -> Result<Loaded, LoadingError> {
        if n == literal(&["prelude"]) { Ok(Loaded::Module(PRELUDE.to_string())) }
        else { file_loader(cwd.clone())(n) }
    }, vliteral(&["...", ">>", ">>=", "[", "]", ",", "=", "=>"]));
    let rules = match collect_rules.try_find(&literal(&["main"])) {
        Ok(rules) => rules,
        Err(err) => panic!("{:#?}", err)
    };
    let mut tree = initial_tree();
    println!("Start processing {tree:?}");
    let repo = Repository::new(rules.as_ref().to_owned());
    println!("Ruleset: {repo:?}");
    xloop!(let mut i = 0; i < 10; i += 1; {
        match repo.step(Mrc::clone(&tree)) {
            Ok(Some(phase)) => {
                println!("Step {i}: {phase:?}");
                tree = phase;
            },
            Ok(None) => {
                println!("Execution complete");
                break
            },
            Err(e) => panic!("Rule error: {e:?}")
        }
    }; println!("Macro execution didn't halt"));
}

fn main() {
    // lambda_notation_debug();
    load_project();
}
