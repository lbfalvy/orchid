mod cli;

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process;

use clap::Parser;
use itertools::Itertools;
use orchidlang::facade::{Environment, PreMacro};
use orchidlang::interner::InternedDisplay;
use orchidlang::systems::stl::StlConfig;
use orchidlang::systems::{io_system, AsynchConfig, IOStream};
use orchidlang::{ast, interpreted, interpreter, Interner, Sym, VName};

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

pub fn to_vname(data: &str, i: &Interner) -> VName {
  data.split("::").map(|s| i.i(s)).collect::<Vec<_>>()
}

fn print_for_debug(e: &ast::Expr<Sym>, i: &Interner) {
  print!(
    "code: {}\nglossary: {}",
    e.bundle(i),
    (e.value.collect_names().into_iter())
      .map(|t| i.extern_vec(t).join("::"))
      .join(", ")
  )
}

/// A little utility to step through the resolution of a macro set
pub fn macro_debug(premacro: PreMacro, sym: Sym, i: &Interner) {
  let (mut code, location) = (premacro.consts.get(&sym))
    .unwrap_or_else(|| {
      panic!(
        "Symbol {} not found\nvalid symbols: \n\t{}\n",
        i.extern_vec(sym).join("::"),
        (premacro.consts.keys())
          .map(|t| i.extern_vec(*t).join("::"))
          .join("\n\t")
      )
    })
    .clone();
  println!(
    "Debugging macros in {} defined at {}.
    Initial state: ",
    i.extern_vec(sym).join("::"),
    location
  );
  print_for_debug(&code, i);
  let mut steps = premacro.step(sym).enumerate();
  loop {
    let (cmd, _) = cmd_prompt("\ncmd> ").unwrap();
    match cmd.trim() {
      "" | "n" | "next" =>
        if let Some((idx, c)) = steps.next() {
          code = c;
          print!("Step {idx}: ");
          print_for_debug(&code, i);
        } else {
          print!("Halted")
        },
      "p" | "print" => print_for_debug(&code, i),
      "d" | "dump" => print!("Rules: {}", premacro.repo.bundle(i)),
      "q" | "quit" => return,
      "h" | "help" => print!(
        "Available commands:
        \t<blank>, n, next\t\ttake a step
        \tp, print\t\tprint the current state
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

pub fn main() {
  let args = Args::parse();
  args.chk_proj().unwrap_or_else(|e| panic!("{e}"));
  let dir = PathBuf::try_from(args.dir).unwrap();
  let i = Interner::new();
  let main = to_vname(&args.main, &i);
  let mut asynch = AsynchConfig::new();
  let io = io_system(&mut asynch, None, None, [
    ("stdin", IOStream::Source(BufReader::new(Box::new(std::io::stdin())))),
    ("stdout", IOStream::Sink(Box::new(std::io::stdout()))),
    ("stderr", IOStream::Sink(Box::new(std::io::stderr()))),
  ]);
  let env = Environment::new(&i)
    .add_system(StlConfig { impure: true })
    .add_system(asynch)
    .add_system(io);
  let premacro = i.unwrap(env.load_dir(&dir, &main));
  if args.dump_repo {
    println!("Parsed rules: {}", premacro.repo.bundle(&i));
    return;
  }
  if !args.macro_debug.is_empty() {
    let sym = i.i(&to_vname(&args.macro_debug, &i));
    return macro_debug(premacro, sym, &i);
  }
  let mut proc = i.unwrap(premacro.build_process(Some(args.macro_limit)));
  let main = interpreted::Clause::Constant(i.i(&main)).wrap();
  let ret = i.unwrap(proc.run(main, None));
  let interpreter::Return { gas, state, inert } = ret;
  drop(proc);
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
