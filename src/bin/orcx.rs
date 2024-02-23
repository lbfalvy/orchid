mod cli;
mod features;

use std::fs::File;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use hashbrown::HashSet;
use itertools::Itertools;
use never::Never;
use orchidlang::error::Reporter;
use orchidlang::facade::macro_runner::MacroRunner;
use orchidlang::facade::merge_trees::{merge_trees, NortConst};
use orchidlang::facade::process::Process;
use orchidlang::foreign::inert::Inert;
use orchidlang::gen::tpl;
use orchidlang::gen::traits::Gen;
use orchidlang::interpreter::gen_nort::nort_gen;
use orchidlang::interpreter::nort::{self};
use orchidlang::libs::std::exit_status::OrcExitStatus;
use orchidlang::libs::std::string::OrcString;
use orchidlang::location::{CodeGenInfo, CodeLocation, SourceRange};
use orchidlang::name::Sym;
use orchidlang::parse::context::FlatLocContext;
use orchidlang::parse::lexer::{lex, Lexeme};
use orchidlang::sym;
use orchidlang::tree::{ModMemberRef, TreeTransforms};
use orchidlang::virt_fs::{decl_file, DeclTree};

use crate::features::macro_debug;
use crate::features::print_project::{print_proj_mod, ProjPrintOpts};
use crate::features::shared::{stderr_sink, stdout_sink, unwrap_exit, with_env, with_std_env};
use crate::features::tests::{get_tree_tests, mock_source, run_test, run_tests, with_mock_env};

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
    #[arg(long, short)]
    symbol: String,
  },
  ListMacros,
  ProjectTree {
    #[arg(long, default_value_t = false)]
    hide_locations: bool,
    #[arg(long)]
    width: Option<u16>,
  },
  Repl,
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

pub fn main() -> ExitCode {
  let args = Args::parse();
  unwrap_exit!(args.chk_proj());
  let dir = PathBuf::from(args.dir);
  let main_s = args.main.as_ref().map_or("tree::main::main", |s| s);
  let main = Sym::parse(main_s).expect("--main cannot be empty");
  let location = CodeLocation::new_gen(CodeGenInfo::no_details(sym!(orcx::entrypoint)));
  let reporter = Reporter::new();

  // subcommands
  #[allow(clippy::blocks_in_conditions)]
  match args.command {
    Some(Command::ListMacros) => with_mock_env(|env| {
      let tree = env.load_main(dir, [main], &reporter);
      let mr = MacroRunner::new(&tree, None, &reporter);
      println!("Parsed rules: {}", mr.repo);
      ExitCode::SUCCESS
    }),
    Some(Command::ProjectTree { hide_locations, width }) => {
      let tree = with_mock_env(|env| env.load_main(dir, [main], &reporter));
      let w = width.or_else(|| termsize::get().map(|s| s.cols)).unwrap_or(74);
      let print_opts = ProjPrintOpts { width: w, hide_locations };
      println!("Project tree: {}", print_proj_mod(&tree.0, 0, print_opts));
      ExitCode::SUCCESS
    },
    Some(Command::MacroDebug { symbol }) => with_mock_env(|env| {
      let tree = env.load_main(dir, [main], &reporter);
      let symbol = Sym::parse(&symbol).expect("macro-debug needs an argument");
      macro_debug::main(tree, symbol).code()
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
      let tree_tests = reporter.unwrap_exit(get_tree_tests(&dir, &reporter));
      unwrap_exit!(run_tests(&dir, args.macro_limit, threads, &tree_tests));
      ExitCode::SUCCESS
    },
    Some(Command::Test { only: Some(symbol), threads: None, system: None }) => {
      let symbol = Sym::parse(&symbol).expect("Test needs an argument");
      with_env(mock_source(), stdout_sink(), stderr_sink(), |env| {
        // iife in lieu of try blocks
        let tree = env.load_main(dir.clone(), [symbol.clone()], &reporter);
        let mr = MacroRunner::new(&tree, Some(args.macro_limit), &reporter);
        let consts = mr.run_macros(tree, &reporter).all_consts();
        let test = consts.get(&symbol).expect("Test not found");
        let nc = NortConst::convert_from(test.clone(), &reporter);
        let mut proc = Process::new(merge_trees(consts, env.systems(), &reporter), env.handlers());
        unwrap_exit!(run_test(&mut proc, symbol.clone(), nc.clone()));
        ExitCode::SUCCESS
      })
    },
    Some(Command::Test { only: None, threads, system: Some(system) }) => {
      let subtrees = unwrap_exit!(with_mock_env(|env| {
        match env.systems().find(|s| s.name == system) {
          None => Err(format!("System {system} not found")),
          Some(sys) => {
            let mut paths = HashSet::new();
            sys.code.search_all((), |path, node, ()| {
              if matches!(node, ModMemberRef::Item(_)) {
                let name = Sym::new(path.unreverse()).expect("Empty path means global file");
                paths.insert(name);
              }
            });
            Ok(paths)
          },
        }
      }));
      let in_subtrees = |sym: Sym| subtrees.iter().any(|sub| sym[..].starts_with(&sub[..]));
      let tests = with_mock_env(|env| {
        let tree = env.load_main(dir.clone(), [main.clone()], &reporter);
        let mr = MacroRunner::new(&tree, Some(args.macro_limit), &reporter);
        let src_consts = mr.run_macros(tree, &reporter).all_consts();
        let consts = merge_trees(src_consts, env.systems(), &reporter);
        (consts.into_iter())
          .filter(|(k, v)| in_subtrees(k.clone()) && v.comments.iter().any(|c| c.trim() == "test"))
          .collect_vec()
      });
      eprintln!("Running {} tests", tests.len());
      unwrap_exit!(run_tests(&dir, args.macro_limit, threads, &tests));
      eprintln!("All tests pass");
      ExitCode::SUCCESS
    },
    None => with_std_env(|env| {
      let proc = env.proc_main(dir, [main.clone()], true, Some(args.macro_limit), &reporter);
      reporter.assert_exit();
      let ret = unwrap_exit!(proc.run(nort::Clause::Constant(main).into_expr(location), None));
      drop(proc);
      match ret.clone().downcast() {
        Ok(Inert(OrcExitStatus::Success)) => ExitCode::SUCCESS,
        Ok(Inert(OrcExitStatus::Failure)) => ExitCode::FAILURE,
        Err(_) => {
          println!("{}", ret.clause);
          ExitCode::SUCCESS
        },
      }
    }),
    Some(Command::Repl) => with_std_env(|env| {
      let sctx = env.project_ctx(&reporter);
      loop {
        let reporter = Reporter::new();
        print!("orc");
        let mut src = String::new();
        let mut paren_tally = 0;
        loop {
          print!("> ");
          stdout().flush().unwrap();
          let mut buf = String::new();
          stdin().read_line(&mut buf).unwrap();
          src += &buf;
          let range = SourceRange::mock();
          let spctx = sctx.parsing(range.code());
          let pctx = FlatLocContext::new(&spctx, &range);
          let res =
            lex(Vec::new(), &buf, &pctx, |_| Ok::<_, Never>(false)).unwrap_or_else(|e| match e {});
          res.tokens.iter().for_each(|e| match &e.lexeme {
            Lexeme::LP(_) => paren_tally += 1,
            Lexeme::RP(_) => paren_tally -= 1,
            _ => (),
          });
          if 0 == paren_tally {
            break;
          }
        }
        let tree = env.load_project_main(
          [sym!(tree::main::__repl_input__)],
          DeclTree::ns("tree::main", [decl_file(&format!("const __repl_input__ := {src}"))]),
          &reporter,
        );
        let mr = MacroRunner::new(&tree, Some(args.macro_limit), &reporter);
        let proj_consts = mr.run_macros(tree, &reporter).all_consts();
        let consts = merge_trees(proj_consts, env.systems(), &reporter);
        let ctx = nort_gen(location.clone());
        let to_string_tpl = tpl::A(tpl::C("std::string::convert"), tpl::Slot);
        if let Err(err) = reporter.bind() {
          eprintln!("{err}");
          continue;
        }
        let proc = Process::new(consts, env.handlers());
        let prompt = tpl::C("tree::main::__repl_input__").template(ctx.clone(), []);
        let out = match proc.run(prompt, Some(1000)) {
          Ok(out) => out,
          Err(e) => {
            eprintln!("{e}");
            continue;
          },
        };
        if let Ok(out) = proc.run(to_string_tpl.template(ctx, [out.clone()]), Some(1000)) {
          if let Ok(s) = out.clone().downcast::<Inert<OrcString>>() {
            println!("{}", s.0.as_str());
            continue;
          }
        }
        println!("{out}")
      }
    }),
  }
}
