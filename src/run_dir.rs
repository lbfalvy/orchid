use std::path::Path;
use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::external::handle;
use crate::interpreter::Return;
use crate::representations::{ast_to_postmacro, postmacro_to_interpreted};
use crate::{external, xloop, interpreter};
use crate::pipeline::{from_const_tree, ProjectTree, parse_layer, collect_rules, collect_consts};
use crate::pipeline::file_loader::{Loaded, mk_cache};
use crate::representations::sourcefile::{FileEntry, Import};
use crate::rule::Repo;
use crate::interner::{Token, Interner, InternedDisplay};

static PRELUDE_TXT:&str = r#"
import std::(
  add, subtract, multiply, remainder, divide,
  equals, ifthenelse,
  concatenate
)

export ...$a + ...$b =0x2p36=> (add (...$a) (...$b))
export ...$a - ...$b:1 =0x2p36=> (subtract (...$a) (...$b))
export ...$a * ...$b =0x1p36=> (multiply (...$a) (...$b))
export ...$a % ...$b:1 =0x1p36=> (remainder (...$a) (...$b))
export ...$a / ...$b:1 =0x1p36=> (divide (...$a) (...$b))
export ...$a == ...$b =0x3p36=> (equals (...$a) (...$b))
export ...$a ++ ...$b =0x4p36=> (concatenate (...$a) (...$b))

export do { ...$statement ; ...$rest:1 } =0x2p130=> statement (...$statement) do { ...$rest }
export do { ...$return } =0x1p130=> ...$return

export statement (let $name = ...$value) ...$next =0x1p230=> (
  (\$name. ...$next) (...$value)
)
export statement (cps $name = ...$operation) ...$next =0x2p230=> (
  (...$operation) \$name. ...$next
)
export statement (cps ...$operation) ...$next =0x1p230=> (
  (...$operation) (...$next)
)

export if ...$cond then ...$true else ...$false:1 =0x1p84=> (
  ifthenelse (...$cond) (...$true) (...$false)
)

export ::(,)
"#;

fn prelude_path(i: &Interner) -> Token<Vec<Token<String>>>
{ i.i(&[ i.i("prelude") ][..]) }
fn mainmod_path(i: &Interner) -> Token<Vec<Token<String>>>
{ i.i(&[ i.i("main") ][..]) }
fn entrypoint(i: &Interner) -> Token<Vec<Token<String>>>
{ i.i(&[ i.i("main"), i.i("main") ][..]) }

fn load_environment(i: &Interner) -> ProjectTree {
  let env = from_const_tree(HashMap::from([
    (i.i("std"), external::std::std(i))
  ]), &[i.i("std")], i);
  let loader = |path: Token<Vec<Token<String>>>| {
    if path == prelude_path(i) {
      Ok(Loaded::Code(Rc::new(PRELUDE_TXT.to_string())))
    } else {
      panic!(
        "Prelude pointed to non-std path {}",
        i.extern_vec(path).join("::")
      )
    }
  };
  parse_layer(&[prelude_path(i)], &loader, &env, &[], i)
    // .unwrap_or_else(|e| panic!("Prelude error: \n {}", e))
    .expect("prelude error")
}

fn load_dir(i: &Interner, dir: &Path) -> ProjectTree {
  let environment = load_environment(i);
  let file_cache = mk_cache(dir.to_path_buf(), i);
  let loader = |path| file_cache.find(&path);
  let prelude = [FileEntry::Import(vec![Import{
    path: prelude_path(i), name: None
  }])];
  parse_layer(&[mainmod_path(i)], &loader, &environment, &prelude, i)
    .expect("Failed to load source code")
}

#[allow(unused)]
pub fn run_dir(dir: &Path) {
  let i = Interner::new();
  let project = load_dir(&i, dir);
  let rules = collect_rules(&project);
  let consts = collect_consts(&project, &i);
  println!("Initializing rule repository with {} rules", rules.len());
  let repo = Repo::new(rules, &i)
    .unwrap_or_else(|(rule, error)| {
      panic!("Rule error: {}
        Offending rule: {}",
        error.bundle(&i),
        rule.bundle(&i)
      )
    });
  println!("Repo dump: {}", repo.bundle(&i));
  let mut exec_table = HashMap::new();
  for (name, source) in consts.iter() {
    // let nval = entrypoint(&i); let name = &nval; let source = &consts[name];
    let mut tree = source.clone();
    let displayname = i.extern_vec(*name).join("::");
    let macro_timeout = 100;
    println!("Executing macros in {displayname}...", );
    let unmatched = xloop!(let mut idx = 0; idx < macro_timeout; idx += 1; {
      match repo.step(&tree) {
        None => break tree,
        Some(phase) => {
          println!("Step {idx}/{macro_timeout}: {}", phase.bundle(&i));
          tree = phase;
        },
      }
    }; panic!("Macro execution in {displayname} didn't halt"));
    let pmtree = ast_to_postmacro::expr(&unmatched)
      .unwrap_or_else(|e| panic!("Postmacro conversion error: {e}"));
    let runtree = postmacro_to_interpreted::expr(&pmtree);
    exec_table.insert(*name, runtree);
  }
  println!("macro execution complete");
  let ctx = interpreter::Context {
    symbols: &exec_table,
    interner: &i,
    gas: None
  };
  let entrypoint = exec_table.get(&entrypoint(&i))
    .unwrap_or_else(|| {
      panic!("entrypoint not found, known keys are: {}",
        exec_table.keys()
          .map(|t| i.r(*t).iter().map(|t| i.r(*t)).join("::"))
          .join(", ")
      )
    });
  let io_handler = handle;
  let ret = interpreter::run_handler(entrypoint.clone(), io_handler, ctx);
  let Return{ gas, state, inert } = ret
    .unwrap_or_else(|e| panic!("Runtime error: {}", e));
  if inert {
    println!("Settled at {}", state.expr().clause.bundle(&i));
    println!("Remaining gas: {}",
      gas.map(|g| g.to_string())
        .unwrap_or(String::from("∞"))
    );
  }
  if gas == Some(0) {println!("Ran out of gas!")}
}