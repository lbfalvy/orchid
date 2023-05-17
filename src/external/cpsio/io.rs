use std::io::{self, Write, stdin};

use crate::{representations::{interpreted::{ExprInst, Clause}, Primitive, Literal}, atomic_inert, interpreter::{HandlerParm, HandlerRes}, unwrap_or, external::runtime_error::RuntimeError};

#[derive(Clone, Debug)]
pub enum IO {
  Print(String, ExprInst),
  Readline(ExprInst)
}
atomic_inert!(IO);

pub fn handle(effect: HandlerParm) -> HandlerRes {
  let io: &IO = unwrap_or!(
    effect.as_any().downcast_ref();
    return Err(effect)
  );
  match io {
    IO::Print(str, cont) => {
      print!("{}", str);
      io::stdout().flush().unwrap();
      Ok(Ok(cont.clone()))
    },
    IO::Readline(cont) => {
      let mut buf = String::new();
      if let Err(e) = stdin().read_line(&mut buf) {
        return Ok(Err(RuntimeError::ext(e.to_string(), "reading from stdin")));
      }
      buf.pop();
      Ok(Ok(Clause::Apply {
        f: cont.clone(),
        x: Clause::P(Primitive::Literal(Literal::Str(buf))).wrap()
      }.wrap()))
    }
  }
}