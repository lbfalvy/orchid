use std::{fs::File, io::Write};

pub use orchid_api::logging::LogStrategy;

pub struct Logger(LogStrategy);
impl Logger {
  pub fn new(strat: LogStrategy) -> Self { Self(strat) }
  pub fn log(&self, msg: String) {
    match &self.0 {
      LogStrategy::StdErr => eprintln!("{msg}"),
      LogStrategy::File(f) => writeln!(File::open(f).unwrap(), "{msg}").unwrap(),
    }
  }
  pub fn strat(&self) -> LogStrategy { self.0.clone() }
}

