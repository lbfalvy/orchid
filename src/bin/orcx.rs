mod cli;
mod features;

use std::fs::File;
use std::io::BufReader;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread::available_parallelism;

use clap::{Parser, Subcommand};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use orchidlang::error::{ProjectError, ProjectErrorObj, ProjectResult};
use orchidlang::facade::loader::Loader;
use orchidlang::facade::macro_runner::MacroRunner;
use orchidlang::facade::merge_trees::merge_trees;
use orchidlang::facade::process::Process;
use orchidlang::foreign::inert::Inert;
use orchidlang::interpreter::context::Halt;
use orchidlang::interpreter::nort;
use orchidlang::libs::asynch::system::AsynchSystem;
use orchidlang::libs::directfs::DirectFS;
use orchidlang::libs::io::{IOService, Stream};
use orchidlang::libs::scheduler::system::SeqScheduler;
use orchidlang::libs::std::exit_status::ExitStatus;
use orchidlang::libs::std::std_system::StdConfig;
use orchidlang::location::{CodeGenInfo, CodeLocation};
use orchidlang::name::Sym;
use orchidlang::tree::{ModMemberRef, TreeTransforms};
use rayon::prelude::ParallelIterator;
use rayon::slice::ParallelSlice;

use crate::features::macro_debug;
use crate::features::print_project::{print_proj_mod, ProjPrintOpts};

#[derive(Subcommand, Debug)]
enum Command {
  /// Run unit tests, any constant annotated --[[ test ]]--
  Test {
    /// Specify an exact test to run
    #[arg(long)]
    only: Option<String>,
    #[arg(long, short)]
    threads: Option<usize>,
    #[arg(long)]
    system: Option<String>,
  },
  #[command(arg_required_else_help = true)]
  MacroDebug {
    symbol: String,
  },
  ListMacros,
  ProjectTree {
    #[arg(long, default_value_t = false)]
    hide_locations: bool,
    #[arg(long)]
    width: Option<u16>,
  },
}
/// Orchid interpreter
#[derive(Parser, Debug)]
#[command(name = "Orchid Executor")]
#[command(author = "Lawrence Bethlenfalvy <lbfalvy@protonmail.com>")]
#[command(long_about = Some("Execute Orchid projects from the file system"))]
struct Args {
  /// Folder containing main.orc or the manually specified entry module
  #[arg(short, long, default_value = ".")]
  pub dir: String,
  /// Alternative entrypoint for the interpreter
  #[arg(short, long)]
  pub main: Option<String>,
  /// Maximum number of steps taken by the macro executor
  #[arg(long, default_value_t = 10_000)]
  pub macro_limit: usize,

  #[command(subcommand)]
  pub command: Option<Command>,
}
impl Args {
  /// Validate the project directory and the
  pub fn chk_dir_main(&self) -> Result<(), String> {
    let dir_path = PathBuf::from(&self.dir);
    if !dir_path.is_dir() {
      return Err(format!("{} is not a directory", dir_path.display()));
    }
    let segs = match &self.main {
      Some(s) => s.split("::").collect::<Vec<_>>(),
      None => match File::open("./main.orc") {
        Ok(_) => return Ok(()),
        Err(e) => return Err(format!("Cannot open './main.orc'\n{e}")),
      },
    };
    if segs.len() < 2 {
      return Err("Entry point too short".to_string());
    };
    let (_, pathsegs) = segs.split_last().unwrap();
    let mut possible_files = pathsegs.iter().scan(dir_path, |path, seg| {
      path.push(seg);
      Some(path.with_extension("orc"))
    });
    if possible_files.all(|p| File::open(p).is_err()) {
      let out_path = pathsegs.join("::");
      let pbuf = PathBuf::from(&self.dir);
      return Err(format!("{out_path} not found in {}", pbuf.display()));
    }
    Ok(())
  }

  pub fn chk_proj(&self) -> Result<(), String> { self.chk_dir_main() }
}

macro_rules! unwrap_exit {
  ($param:expr) => {
    match $param {
      Ok(v) => v,
      Err(e) => {
        eprintln!("{e}");
        return ExitCode::FAILURE;
      },
    }
  };
}

pub fn with_std_proc<T>(
  dir: &Path,
  macro_limit: usize,
  f: impl for<'a> FnOnce(Process<'a>) -> ProjectResult<T>,
) -> ProjectResult<T> {
  with_std_env(|env| {
    let mr = MacroRunner::new(&env.load_dir(dir.to_owned())?)?;
    let source_syms = mr.run_macros(Some(macro_limit))?;
    let consts = merge_trees(source_syms, env.systems())?;
    let proc = Process::new(consts, env.handlers());
    f(proc)
  })
}

// TODO
pub fn run_test(proc: &mut Process, name: Sym) -> ProjectResult<()> { Ok(()) }
pub fn run_tests(
  dir: &Path,
  macro_limit: usize,
  threads: Option<usize>,
  tests: &[Sym],
) -> ProjectResult<()> {
  with_std_proc(dir, macro_limit, |proc| proc.validate_refs())?;
  let threads = threads
    .or_else(|| available_parallelism().ok().map(NonZeroUsize::into))
    .unwrap_or(1);
  rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().unwrap();
  let batch_size = tests.len().div_ceil(threads);
  let errors = tests
    .par_chunks(batch_size)
    .map(|tests| {
      let res = with_std_proc(dir, macro_limit, |mut proc| {
        let mut errors = HashMap::new();
        for test in tests {
          if let Err(e) = run_test(&mut proc, test.clone()) {
            errors.insert(test.clone(), e);
          }
        }
        Ok(errors)
      });
      res.expect("Tested earlier")
    })
    .reduce(HashMap::new, |l, r| l.into_iter().chain(r).collect());
  if errors.is_empty() { Ok(()) } else { Err(TestsFailed(errors).pack()) }
}

pub struct TestsFailed(HashMap<Sym, ProjectErrorObj>);
impl ProjectError for TestsFailed {
  const DESCRIPTION: &'static str = "Various tests failed";
  fn message(&self) -> String {
    format!(
      "{} tests failed. Errors:\n{}",
      self.0.len(),
      self.0.iter().map(|(k, e)| format!("In {k}, {e}")).join("\n")
    )
  }
}

fn get_tree_tests(dir: &Path) -> ProjectResult<Vec<Sym>> {
  with_std_env(|env| {
    env.load_dir(dir.to_owned()).map(|tree| {
      (tree.all_consts().into_iter())
        .filter(|(_, rep)| rep.comments.iter().any(|s| s.trim() == "test"))
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>()
    })
  })
}

pub fn with_std_env<T>(cb: impl for<'a> FnOnce(Loader<'a>) -> T) -> T {
  let mut asynch = AsynchSystem::new();
  let scheduler = SeqScheduler::new(&mut asynch);
  let std_streams = [
    ("stdin", Stream::Source(BufReader::new(Box::new(std::io::stdin())))),
    ("stdout", Stream::Sink(Box::new(std::io::stdout()))),
    ("stderr", Stream::Sink(Box::new(std::io::stderr()))),
  ];
  let env = Loader::new()
    .add_system(StdConfig { impure: true })
    .add_system(asynch)
    .add_system(scheduler.clone())
    .add_system(IOService::new(scheduler.clone(), std_streams))
    .add_system(DirectFS::new(scheduler));
  cb(env)
}

pub fn main() -> ExitCode {
  let args = Args::parse();
  unwrap_exit!(args.chk_proj());
  let dir = PathBuf::from(args.dir);
  let main = args.main.map_or_else(
    || Sym::literal("tree::main::main"),
    |main| Sym::parse(&main).expect("--main cannot be empty"),
  );

  // subcommands
  match args.command {
    Some(Command::ListMacros) => with_std_env(|env| {
      let tree = unwrap_exit!(env.load_main(dir, main));
      let mr = unwrap_exit!(MacroRunner::new(&tree));
      println!("Parsed rules: {}", mr.repo);
      ExitCode::SUCCESS
    }),
    Some(Command::ProjectTree { hide_locations, width }) => {
      let tree = unwrap_exit!(with_std_env(|env| env.load_main(dir, main)));
      let w = width.or_else(|| termsize::get().map(|s| s.cols)).unwrap_or(74);
      let print_opts = ProjPrintOpts { width: w, hide_locations };
      println!("Project tree: {}", print_proj_mod(&tree.0, 0, print_opts));
      ExitCode::SUCCESS
    },
    Some(Command::MacroDebug { symbol }) => with_std_env(|env| {
      let tree = unwrap_exit!(env.load_main(dir, main));
      let symbol = Sym::parse(&symbol).expect("macro-debug needs an argument");
      macro_debug::main(unwrap_exit!(MacroRunner::new(&tree)), symbol).code()
    }),
    Some(Command::Test { only: Some(_), threads: Some(_), .. }) => {
      eprintln!(
        "Each test case runs in a single thread.
        --only and --threads cannot both be specified"
      );
      ExitCode::FAILURE
    },
    Some(Command::Test { only: Some(_), system: Some(_), .. }) => {
      eprintln!(
        "Conflicting test filters applied. --only runs a single test by
        symbol name, while --system runs all tests in a system"
      );
      ExitCode::FAILURE
    },
    Some(Command::Test { only: None, threads, system: None }) => {
      let tree_tests = unwrap_exit!(get_tree_tests(&dir));
      unwrap_exit!(run_tests(&dir, args.macro_limit, threads, &tree_tests));
      ExitCode::SUCCESS
    },
    Some(Command::Test { only: Some(symbol), threads: None, system: None }) => {
      let symbol = Sym::parse(&symbol).expect("Test needs an argument");
      unwrap_exit!(run_tests(&dir, args.macro_limit, Some(1), &[symbol]));
      ExitCode::SUCCESS
    },
    Some(Command::Test { only: None, threads, system: Some(system) }) => {
      let subtrees = unwrap_exit!(with_std_env(|env| {
        match env.systems().find(|s| s.name == system) {
          None => Err(format!("System {system} not found")),
          Some(sys) => {
            let mut paths = HashSet::new();
            sys.code.search_all((), |path, node, ()| {
              if matches!(node, ModMemberRef::Item(_)) {
                let name = Sym::new(path.unreverse())
                  .expect("Empty path means global file");
                paths.insert(name);
              }
            });
            Ok(paths)
          },
        }
      }));
      let in_subtrees =
        |sym: Sym| subtrees.iter().any(|sub| sym[..].starts_with(&sub[..]));
      let tests = unwrap_exit!(with_std_env(|env| -> ProjectResult<_> {
        let tree = env.load_main(dir.clone(), main.clone())?;
        let mr = MacroRunner::new(&tree)?;
        let src_consts = mr.run_macros(Some(args.macro_limit))?;
        let consts = merge_trees(src_consts, env.systems())?;
        let test_names = (consts.into_iter())
          .filter(|(k, v)| {
            in_subtrees(k.clone())
              && v.comments.iter().any(|c| c.trim() == "test")
          })
          .map(|p| p.0)
          .collect_vec();
        Ok(test_names)
      }));
      eprintln!("Running {} tests", tests.len());
      unwrap_exit!(run_tests(&dir, args.macro_limit, threads, &tests));
      eprintln!("All tests pass");
      ExitCode::SUCCESS
    },
    None => with_std_env(|env| {
      let tree = unwrap_exit!(env.load_main(dir, main.clone()));
      let mr = unwrap_exit!(MacroRunner::new(&tree));
      let src_consts = unwrap_exit!(mr.run_macros(Some(args.macro_limit)));
      let consts = unwrap_exit!(merge_trees(src_consts, env.systems()));
      let mut proc = Process::new(consts, env.handlers());
      unwrap_exit!(proc.validate_refs());
      let main = nort::Clause::Constant(main.clone())
        .to_expr(CodeLocation::Gen(CodeGenInfo::no_details("entrypoint")));
      let ret = unwrap_exit!(proc.run(main, None));
      let Halt { state, inert, .. } = ret;
      drop(proc);
      assert!(inert, "Gas is not used, only inert data should be yielded");
      match state.clone().downcast() {
        Ok(Inert(ExitStatus::Success)) => ExitCode::SUCCESS,
        Ok(Inert(ExitStatus::Failure)) => ExitCode::FAILURE,
        Err(_) => {
          println!("{}", state.clause);
          ExitCode::SUCCESS
        },
      }
    }),
  }
}
