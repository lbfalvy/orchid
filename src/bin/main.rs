mod cli;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use hashbrown::HashMap;
use itertools::Itertools;
use orchidlang::interner::{InternedDisplay, Interner, Sym};
use orchidlang::{ast, ast_to_interpreted, interpreter, pipeline, rule, stl};

use crate::cli::cmd_prompt;

/// Orchid interpreter
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
  /// Folder containing main.orc or the manually specified entry module
  #[arg(short, long, default_value = ".")]
  pub dir: String,
  /// Entrypoint for the interpreter
  #[arg(short, long, default_value = "main::main")]
  pub main: String,
  /// Maximum number of steps taken by the macro executor
  #[arg(long, default_value_t = 10_000)]
  pub macro_limit: usize,
  /// Print the parsed ruleset and exit
  #[arg(long)]
  pub dump_repo: bool,
  /// Step through the macro execution process in the specified symbol
  #[arg(long, default_value = "")]
  pub macro_debug: String,
}
impl Args {
  /// Validate the project directory and the
  pub fn chk_dir_main(&self) -> Result<(), String> {
    let dir_path = PathBuf::from(&self.dir);
    if !dir_path.is_dir() {
      return Err(format!("{} is not a directory", dir_path.display()));
    }
    let segs = self.main.split("::").collect::<Vec<_>>();
    if segs.len() < 2 {
      return Err("Entry point too short".to_string());
    }
    let (pathsegs, _) = segs.split_at(segs.len() - 1);
    let mut possible_files = pathsegs.iter().scan(dir_path, |path, seg| {
      path.push(seg);
      Some(path.with_extension("orc"))
    });
    if possible_files.all(|p| File::open(p).is_err()) {
      return Err(format!(
        "{} not found in {}",
        pathsegs.join("::"),
        PathBuf::from(&self.dir).display()
      ));
    }
    Ok(())
  }

  pub fn chk_proj(&self) -> Result<(), String> {
    self.chk_dir_main()
  }
}

/// Load and parse all source related to the symbol `target` or all symbols
/// in the namespace `target` in the context of the STL. All sourcefiles must
/// reside within `dir`.
fn load_dir(dir: &Path, target: Sym, i: &Interner) -> pipeline::ProjectTree {
  let file_cache = pipeline::file_loader::mk_dir_cache(dir.to_path_buf(), i);
  let library = stl::mk_stl(i, stl::StlOptions::default());
  pipeline::parse_layer(
    &[target],
    &|path| file_cache.find(&path),
    &library,
    &stl::mk_prelude(i),
    i,
  )
  .expect("Failed to load source code")
}

pub fn to_sym(data: &str, i: &Interner) -> Sym {
  i.i(&data.split("::").map(|s| i.i(s)).collect::<Vec<_>>()[..])
}

/// A little utility to step through the resolution of a macro set
pub fn macro_debug(repo: rule::Repo, mut code: ast::Expr, i: &Interner) {
  let mut idx = 0;
  println!("Macro debugger working on {}", code.bundle(i));
  loop {
    let (cmd, _) = cmd_prompt("cmd> ").unwrap();
    match cmd.trim() {
      "" | "n" | "next" =>
        if let Some(c) = repo.step(&code) {
          idx += 1;
          code = c;
          println!("Step {idx}: {}", code.bundle(i));
        },
      "p" | "print" => println!("Step {idx}: {}", code.bundle(i)),
      "d" | "dump" => println!("Rules: {}", repo.bundle(i)),
      "q" | "quit" => return,
      "h" | "help" => println!(
        "Available commands:
        \t<blank>, n, next\t\ttake a step
        \tp, print\t\tprint the current state
        \tq, quit\t\texit
        \th, help\t\tprint this text"
      ),
      _ => {
        println!("unrecognized command \"{}\", try \"help\"", cmd);
        continue;
      },
    }
  }
}

pub fn main() {
  let args = Args::parse();
  args.chk_proj().unwrap_or_else(|e| panic!("{e}"));
  let dir = PathBuf::try_from(args.dir).unwrap();
  let i = Interner::new();
  let main = to_sym(&args.main, &i);
  let project = load_dir(&dir, main, &i);
  let rules = pipeline::collect_rules(&project);
  let consts = pipeline::collect_consts(&project, &i);
  let repo = rule::Repo::new(rules, &i).unwrap_or_else(|(rule, error)| {
    panic!(
      "Rule error: {}
      Offending rule: {}",
      error.bundle(&i),
      rule.bundle(&i)
    )
  });
  if args.dump_repo {
    println!("Parsed rules: {}", repo.bundle(&i));
    return;
  } else if !args.macro_debug.is_empty() {
    let name = to_sym(&args.macro_debug, &i);
    let code = consts
      .get(&name)
      .unwrap_or_else(|| panic!("Constant {} not found", args.macro_debug));
    return macro_debug(repo, code.clone(), &i);
  }
  let mut exec_table = HashMap::new();
  for (name, source) in consts.iter() {
    let displayname = i.extern_vec(*name).join("::");
    let (unmatched, steps_left) = repo.long_step(source, args.macro_limit + 1);
    assert!(steps_left > 0, "Macro execution in {displayname} did not halt");
    let runtree = ast_to_interpreted(&unmatched).unwrap_or_else(|e| {
      panic!("Postmacro conversion error in {displayname}: {e}")
    });
    exec_table.insert(*name, runtree);
  }
  let ctx =
    interpreter::Context { symbols: &exec_table, interner: &i, gas: None };
  let entrypoint = exec_table.get(&main).unwrap_or_else(|| {
    let main = args.main;
    let symbols =
      exec_table.keys().map(|t| i.extern_vec(*t).join("::")).join(", ");
    panic!(
      "Entrypoint not found!
      Entrypoint was {main}
      known keys are {symbols}"
    )
  });
  let io_handler = orchidlang::stl::handleIO;
  let ret = interpreter::run_handler(entrypoint.clone(), io_handler, ctx);
  let interpreter::Return { gas, state, inert } =
    ret.unwrap_or_else(|e| panic!("Runtime error: {}", e));
  if inert {
    println!("Settled at {}", state.expr().clause.bundle(&i));
    if let Some(g) = gas {
      println!("Remaining gas: {g}")
    }
  } else if gas == Some(0) {
    eprintln!("Ran out of gas!");
    process::exit(-1);
  }
}
