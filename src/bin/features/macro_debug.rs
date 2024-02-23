use itertools::Itertools;
use orchidlang::error::Reporter;
use orchidlang::facade::macro_runner::MacroRunner;
use orchidlang::libs::std::exit_status::OrcExitStatus;
use orchidlang::location::{CodeGenInfo, CodeLocation};
use orchidlang::name::Sym;
use orchidlang::pipeline::project::{ItemKind, ProjItem, ProjectTree};
use orchidlang::sym;

use crate::cli::cmd_prompt;

/// A little utility to step through the reproject of a macro set
pub fn main(tree: ProjectTree, symbol: Sym) -> OrcExitStatus {
  print!("Macro debugger starting on {symbol}");
  let location = CodeLocation::new_gen(CodeGenInfo::no_details(sym!(orcx::macro_runner)));
  let expr_ent = match tree.0.walk1_ref(&[], &symbol[..], |_| true) {
    Ok((e, _)) => e.clone(),
    Err(e) => {
      eprintln!("{}", e.at(&location.origin()));
      return OrcExitStatus::Failure;
    },
  };
  let mut expr = match expr_ent.item() {
    Some(ProjItem { kind: ItemKind::Const(c) }) => c.clone(),
    _ => {
      eprintln!("macro-debug argument must be a constant");
      return OrcExitStatus::Failure;
    },
  };
  let reporter = Reporter::new();
  let macro_runner = MacroRunner::new(&tree, None, &reporter);
  reporter.assert_exit();
  println!("\nInitial state: {expr}");
  // print_for_debug(&code);
  let mut steps = macro_runner.step(expr.clone()).enumerate();
  loop {
    let (cmd, _) = cmd_prompt("\ncmd> ").unwrap();
    match cmd.trim() {
      "" | "n" | "next" => match steps.next() {
        None => print!("Halted"),
        Some((idx, c)) => {
          expr = c;
          print!("Step {idx}: {expr}");
        },
      },
      "p" | "print" => {
        let glossary = expr.value.collect_names();
        let gl_str = glossary.iter().join(", ");
        print!("code: {expr}\nglossary: {gl_str}")
      },
      "d" | "dump" => print!("Rules: {}", macro_runner.repo),
      "q" | "quit" => return OrcExitStatus::Success,
      "complete" => {
        match steps.last() {
          Some((idx, c)) => print!("Step {idx}: {c}"),
          None => print!("Already halted"),
        }
        return OrcExitStatus::Success;
      },
      "h" | "help" => print!(
        "Available commands:
        \t<blank>, n, next\t\ttake a step
        \tp, print\t\tprint the current state
        \td, dump\t\tprint the rule table
        \tq, quit\t\texit
        \th, help\t\tprint this text"
      ),
      _ => {
        print!("unrecognized command \"{}\", try \"help\"", cmd);
        continue;
      },
    }
  }
}
