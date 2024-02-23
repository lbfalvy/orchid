use std::fmt;
use std::io::BufReader;
use std::path::Path;

use hashbrown::HashMap;
use itertools::Itertools;
use orchidlang::error::{ProjectError, ProjectResult, Reporter};
use orchidlang::facade::loader::Loader;
use orchidlang::facade::macro_runner::MacroRunner;
use orchidlang::facade::merge_trees::NortConst;
use orchidlang::facade::process::Process;
use orchidlang::foreign::error::{RTError, RTErrorObj, RTResult};
use orchidlang::foreign::inert::Inert;
use orchidlang::interpreter::error::RunError;
use orchidlang::interpreter::nort;
use orchidlang::libs::io::{Sink, Source};
use orchidlang::libs::std::exit_status::OrcExitStatus;
use orchidlang::name::Sym;
use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;

use super::shared::{with_env, worker_cnt};

pub fn mock_source() -> Source { BufReader::new(Box::new(&[][..])) }
pub fn mock_sink() -> Sink { Box::<Vec<u8>>::default() }
pub fn with_mock_env<T>(cb: impl for<'a> FnOnce(Loader<'a>) -> T) -> T {
  with_env(mock_source(), mock_sink(), mock_sink(), cb)
}

#[derive(Clone)]
pub struct TestDidNotHalt(Sym);
impl RTError for TestDidNotHalt {}
impl fmt::Display for TestDidNotHalt {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Test {} did not halt", self.0)
  }
}

#[derive(Clone)]
pub struct TestDidNotSucceed(Sym, nort::Expr);
impl RTError for TestDidNotSucceed {}
impl fmt::Display for TestDidNotSucceed {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Test {} settled on {}", self.0, self.1)
  }
}

pub fn run_test(proc: &mut Process, name: Sym, data: NortConst) -> RTResult<()> {
  let res = proc.run(data.value, Some(10_000)).map_err(|e| match e {
    RunError::Extern(e) => e,
    RunError::Interrupted(_) => TestDidNotHalt(name.clone()).pack(),
  })?;
  match res.clone().downcast()? {
    Inert(OrcExitStatus::Success) => Ok(()),
    _ => Err(TestDidNotSucceed(name, res).pack()),
  }
}
pub fn run_tests(
  dir: &Path,
  macro_limit: usize,
  threads: Option<usize>,
  tests: &[(Sym, NortConst)],
) -> ProjectResult<()> {
  with_mock_env(|env| {
    let reporter = Reporter::new();
    env.proc_dir(dir.to_owned(), true, Some(macro_limit), &reporter);
    reporter.bind()
  })?;
  let threads = threads.unwrap_or_else(worker_cnt);
  rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().unwrap();
  let batch_size = tests.len().div_ceil(threads);
  let errors = (tests.par_chunks(batch_size))
    .map(|tests| {
      with_mock_env(|env| {
        let reporter = Reporter::new();
        let mut proc = env.proc_dir(dir.to_owned(), true, Some(macro_limit), &reporter);
        reporter.assert(); // checked above
        (tests.iter())
          .filter_map(|(test, constant)| {
            Some((test.clone(), run_test(&mut proc, test.clone(), constant.clone()).err()?))
          })
          .collect_vec()
      })
    })
    .collect::<Vec<_>>()
    .into_iter()
    .flatten()
    .collect::<HashMap<_, _>>();
  if errors.is_empty() { Ok(()) } else { Err(TestsFailed(errors).pack()) }
}

pub struct TestsFailed(HashMap<Sym, RTErrorObj>);
impl ProjectError for TestsFailed {
  const DESCRIPTION: &'static str = "Various tests failed";
  fn message(&self) -> String {
    ([format!("{} tests failed. Errors:", self.0.len())].into_iter())
      .chain(self.0.iter().map(|(k, e)| format!("In {k}, {e}")))
      .join("\n")
  }
}

pub fn get_tree_tests(dir: &Path, reporter: &Reporter) -> ProjectResult<Vec<(Sym, NortConst)>> {
  with_mock_env(|env| {
    let tree = env.load_dir(dir.to_owned(), reporter);
    let tree = MacroRunner::new(&tree, Some(10_000), reporter).run_macros(tree, reporter);
    (tree.all_consts().into_iter())
      .filter(|(_, rep)| rep.comments.iter().any(|s| s.trim() == "test"))
      .map(|(k, v)| Ok((k.clone(), NortConst::convert_from(v, reporter))))
      .collect::<ProjectResult<Vec<_>>>()
  })
}
