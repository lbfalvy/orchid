#![feature(specialization)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)] 
#![feature(generators, generator_trait)]
#![feature(never_type)]
#![feature(unwrap_infallible)]
#![feature(arc_unwrap_or_clone)]
#![feature(hasher_prefixfree_extras)]
#![feature(closure_lifetime_binder)]
#![feature(generic_arg_infer)]
use std::{env::current_dir, collections::HashMap};

// mod executor;
mod parse;
pub(crate) mod project;
mod utils;
mod representations;
mod rule;
mod scheduler;
pub(crate) mod foreign;
mod external;
mod foreign_macros;
use lasso::Rodeo;
pub use representations::ast;
use ast::{Expr, Clause};
// use representations::typed as t;
use mappable_rc::Mrc;
use project::{rule_collector, file_loader};
use rule::Repository;
use utils::to_mrc_slice;

use crate::external::std::std;
use crate::project::{map_loader, string_loader, Loader, ModuleError};
use crate::representations::{ast_to_postmacro, postmacro_to_interpreted};

fn literal(orig: &[&str]) -> Mrc<[String]> {
  to_mrc_slice(vliteral(orig))
}

fn vliteral(orig: &[&str]) -> Vec<String> {
  orig.iter().map(|&s| s.to_owned()).collect()
}

static PRELUDE:&str = r#"
import std::(
  num::(add, subtract, multiply, remainder, divide),
  bool::(equals, ifthenelse),
  str::concatenate
)

export (...$a + ...$b) =1001=> (add (...$a) (...$b))
export (...$a - ...$b:1) =1001=> (subtract (...$a) (...$b))
export (...$a * ...$b) =1000=> (multiply (...$a) (...$b))
export (...$a % ...$b:1) =1000=> (remainder (...$a) (...$b))
export (...$a / ...$b:1) =1000=> (divide (...$a) (...$b))
export (...$a == ...$b) =1002=> (equals (...$a) (...$b))
export (...$a ++ ...$b) =1003=> (concatenate (...$a) (...$b))

export do { ...$statement ; ...$rest:1 } =10_001=> (
  statement (...$statement) do { ...$rest } 
)
export do { ...$return } =10_000=> (...$return)

export statement (let $_name = ...$value) ...$next =10_000=> (
  (\$_name. ...$next) (...$value)
)
export statement (cps $_name = ...$operation) ...$next =10_001=> (
  (...$operation) \$_name. ...$next
)
export statement (cps ...$operation) ...$next =10_000=> (
  (...$operation) (...$next)
)

export if ...$cond then ...$true else ...$false:1 =5_000=> (
  ifthenelse (...$cond) (...$true) (...$false)
)
"#;

fn initial_tree() -> Mrc<[Expr]> {
  to_mrc_slice(vec![Expr(Clause::Name {
    local: None,
    qualified: literal(&["mod", "main", "main"])
  }, to_mrc_slice(vec![]))])
}

#[allow(unused)]
fn load_project() {
  let mut rodeo = Rodeo::default();
  let collect_rules = rule_collector(
    rodeo,
    map_loader(HashMap::from([
      ("std", std().boxed()),
      ("prelude", string_loader(PRELUDE).boxed()),
      ("mod", file_loader(current_dir().expect("Missing CWD!")).boxed())
    ]))
  );
  let rules = match collect_rules.try_find(&literal(&["mod", "main"])) {
    Ok(rules) => rules,
    Err(err) => if let ModuleError::Syntax(pe) = err {
      panic!("{}", pe);
    } else {panic!("{:#?}", err)}
  };
  let mut tree = initial_tree();
  println!("Start processing {tree:?}");
  let repo = Repository::new(rules.as_ref().to_owned());
  println!("Ruleset: {repo:?}");
  xloop!(let mut i = 0; i < 100; i += 1; {
    match repo.step(Mrc::clone(&tree)) {
      Ok(Some(phase)) => {
        //println!("Step {i}: {phase:?}");
        tree = phase;
      },
      Ok(None) => {
        println!("Execution complete");
        break
      },
      Err(e) => panic!("Rule error: {e:?}")
    }
  }; panic!("Macro execution didn't halt"));
  let pmtree = ast_to_postmacro::exprv(tree.as_ref())
    .unwrap_or_else(|e| panic!("Postmacro conversion error: {e}"));
  let runtree = postmacro_to_interpreted::expr_rec(&pmtree)
    .unwrap_or_else(|e| panic!("Interpreted conversion error: {e}"));
  let stable = runtree.run_to_completion()
    .unwrap_or_else(|e| panic!("Runtime error {e}"));
  println!("Settled at {stable:?}")
}

fn main() {
  load_project();
}
