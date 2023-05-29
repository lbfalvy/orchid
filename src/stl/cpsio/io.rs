use std::io::{self, Write};

use super::super::runtime_error::RuntimeError;
use crate::atomic_inert;
use crate::interpreter::{HandlerParm, HandlerRes};
use crate::representations::interpreted::{Clause, ExprInst};
use crate::representations::{Literal, Primitive};
use crate::utils::unwrap_or;

/// An IO command to be handled by the host application.
#[derive(Clone, Debug)]
pub enum IO {
  /// Print a string to standard output and resume execution
  Print(String, ExprInst),
  /// Read a line from standard input and pass it to the calback
  Readline(ExprInst),
}
atomic_inert!(IO);

/// Default xommand handler for IO actions
pub fn handle(effect: HandlerParm) -> HandlerRes {
  // Downcast command
  let io: &IO = unwrap_or!(effect.as_any().downcast_ref(); Err(effect)?);
  // Interpret and execute
  Ok(match io {
    IO::Print(str, cont) => {
      print!("{}", str);
      io::stdout()
        .flush()
        .map_err(|e| RuntimeError::ext(e.to_string(), "writing to stdout"))?;
      cont.clone()
    },
    IO::Readline(cont) => {
      let mut buf = String::new();
      io::stdin()
        .read_line(&mut buf)
        .map_err(|e| RuntimeError::ext(e.to_string(), "reading from stdin"))?;
      buf.pop();
      let x = Clause::P(Primitive::Literal(Literal::Str(buf))).wrap();
      Clause::Apply { f: cont.clone(), x }.wrap()
    },
  })
}