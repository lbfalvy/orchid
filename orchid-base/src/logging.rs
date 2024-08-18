use std::fmt::Arguments;
use std::fs::File;
use std::io::{stderr, Write};

pub use api::LogStrategy;
use itertools::Itertools;

use crate::api;

#[derive(Clone)]
pub struct Logger(api::LogStrategy);
impl Logger {
  pub fn new(strat: api::LogStrategy) -> Self { Self(strat) }
  pub fn log(&self, msg: impl AsRef<str>) { writeln!(self, "{}", msg.as_ref()) }
  pub fn strat(&self) -> api::LogStrategy { self.0.clone() }
  pub fn log_buf(&self, event: impl AsRef<str>, buf: &[u8]) {
    if !std::env::var("ORCHID_LOG_BUFFERS").unwrap().is_empty() {
      writeln!(self, "{}: [{}]", event.as_ref(), buf.iter().map(|b| format!("{b:02x}")).join(" "))
    }
  }
  pub fn write_fmt(&self, fmt: Arguments) {
    match &self.0 {
      api::LogStrategy::StdErr => stderr().write_fmt(fmt).expect("Could not write to stderr!"),
      api::LogStrategy::File(f) => {
        let mut file = File::open(f).expect("Could not open logfile");
        file.write_fmt(fmt).expect("Could not write to logfile");
      },
    }
  }
}
