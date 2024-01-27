use itertools::Itertools;
use orchidlang::facade::macro_runner::MacroRunner;
use orchidlang::libs::std::exit_status::ExitStatus;
use orchidlang::name::Sym;

use crate::cli::cmd_prompt;

/// A little utility to step through the resolution of a macro set
pub fn main(macro_runner: MacroRunner, sym: Sym) -> ExitStatus {
  let outname = sym.iter().join("::");
  let (mut code, location) = match macro_runner.consts.get(&sym) {
    Some(rep) => (rep.value.clone(), rep.range.clone()),
    None => {
      let valid = macro_runner.consts.keys();
      let valid_str = valid.map(|t| t.iter().join("::")).join("\n\t");
      eprintln!("Symbol {outname} not found\nvalid symbols: \n\t{valid_str}\n");
      return ExitStatus::Failure;
    },
  };
  print!("Debugging macros in {outname} defined at {location}");
  println!("\nInitial state: {code}");
  // print_for_debug(&code);
  let mut steps = macro_runner.step(sym).enumerate();
  loop {
    let (cmd, _) = cmd_prompt("\ncmd> ").unwrap();
    match cmd.trim() {
      "" | "n" | "next" => match steps.next() {
        None => print!("Halted"),
        Some((idx, c)) => {
          code = c;
          print!("Step {idx}: {code}");
        },
      },
      "p" | "print" => {
        let glossary = code.value.collect_names();
        let gl_str = glossary.iter().map(|t| t.iter().join("::")).join(", ");
        print!("code: {code}\nglossary: {gl_str}")
      },
      "d" | "dump" => print!("Rules: {}", macro_runner.repo),
      "q" | "quit" => return ExitStatus::Success,
      "complete" => {
        match steps.last() {
          Some((idx, c)) => print!("Step {idx}: {c}"),
          None => print!("Already halted"),
        }
        return ExitStatus::Success;
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
