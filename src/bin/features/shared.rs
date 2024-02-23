use std::io::BufReader;
use std::thread;

use orchidlang::facade::loader::Loader;
use orchidlang::libs::asynch::system::AsynchSystem;
use orchidlang::libs::directfs::DirectFS;
use orchidlang::libs::io::{IOService, Sink, Source, Stream};
use orchidlang::libs::scheduler::system::SeqScheduler;
use orchidlang::libs::std::std_system::StdConfig;

pub fn stdin_source() -> Source { BufReader::new(Box::new(std::io::stdin())) }
pub fn stdout_sink() -> Sink { Box::new(std::io::stdout()) }
pub fn stderr_sink() -> Sink { Box::new(std::io::stderr()) }

pub fn with_std_env<T>(cb: impl for<'a> FnOnce(Loader<'a>) -> T) -> T {
  with_env(stdin_source(), stdout_sink(), stderr_sink(), cb)
}

pub fn with_env<T>(
  stdin: Source,
  stdout: Sink,
  stderr: Sink,
  cb: impl for<'a> FnOnce(Loader<'a>) -> T,
) -> T {
  let mut asynch = AsynchSystem::new();
  let scheduler = SeqScheduler::new(&mut asynch);
  let std_streams = [
    ("stdin", Stream::Source(stdin)),
    ("stdout", Stream::Sink(stdout)),
    ("stderr", Stream::Sink(stderr)),
  ];
  let env = Loader::new()
    .add_system(StdConfig { impure: true })
    .add_system(asynch)
    .add_system(scheduler.clone())
    .add_system(IOService::new(scheduler.clone(), std_streams))
    .add_system(DirectFS::new(scheduler));
  cb(env)
}

pub fn worker_cnt() -> usize { thread::available_parallelism().map(usize::from).unwrap_or(1) }

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
  ($param:expr; $error:expr) => {
    match $param {
      Ok(v) => v,
      Err(e) => {
        eprintln!("{e}");
        return $error;
      },
    }
  };
}

pub(crate) use unwrap_exit;
