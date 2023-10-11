use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;

use super::flow::IOCmd;
use crate::foreign::Atomic;
use crate::interpreted::ExprInst;
use crate::systems::codegen::call;
use crate::systems::scheduler::{Canceller, SharedHandle};
use crate::systems::stl::Binary;
use crate::OrcString;

/// Any type that we can read controlled amounts of data from
pub type Source = BufReader<Box<dyn Read + Send>>;
/// Any type that we can write data to
pub type Sink = Box<dyn Write + Send>;

/// String reading command
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SRead {
  All,
  Line,
}

/// Binary reading command
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BRead {
  All,
  N(usize),
  Until(u8),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ReadCmd {
  RBytes(BRead),
  RStr(SRead),
}

impl IOCmd for ReadCmd {
  type Stream = Source;
  type Result = ReadResult;
  type Handle = SharedHandle<Source>;

  // This is a buggy rule, check manually
  #[allow(clippy::read_zero_byte_vec)]
  fn execute(
    self,
    stream: &mut Self::Stream,
    _cancel: Canceller,
  ) -> Self::Result {
    match self {
      Self::RBytes(bread) => {
        let mut buf = Vec::new();
        let result = match &bread {
          BRead::All => stream.read_to_end(&mut buf).map(|_| ()),
          BRead::Until(b) => stream.read_until(*b, &mut buf).map(|_| ()),
          BRead::N(n) => {
            buf.resize(*n, 0);
            stream.read_exact(&mut buf)
          },
        };
        ReadResult::RBin(bread, result.map(|_| buf))
      },
      Self::RStr(sread) => {
        let mut buf = String::new();
        let sresult = match &sread {
          SRead::All => stream.read_to_string(&mut buf).map(|_| ()),
          SRead::Line => stream.read_line(&mut buf).map(|_| {
            if buf.ends_with('\n') {
              buf.pop();
            }
          }),
        };
        ReadResult::RStr(sread, sresult.map(|()| buf))
      },
    }
  }
}

/// Reading command (string or binary)
pub enum ReadResult {
  RStr(SRead, io::Result<String>),
  RBin(BRead, io::Result<Vec<u8>>),
}
impl ReadResult {
  pub fn dispatch(self, succ: ExprInst, fail: ExprInst) -> Vec<ExprInst> {
    match self {
      ReadResult::RBin(_, Err(e)) | ReadResult::RStr(_, Err(e)) => {
        vec![call(fail, [wrap_io_error(e)]).wrap()]
      },
      ReadResult::RBin(_, Ok(bytes)) => {
        let arg = Binary(Arc::new(bytes)).atom_cls().wrap();
        vec![call(succ, [arg]).wrap()]
      },
      ReadResult::RStr(_, Ok(text)) => {
        vec![call(succ, [OrcString::from(text).atom_exi()]).wrap()]
      },
    }
  }
}

/// Function to convert [io::Error] to Orchid data
pub fn wrap_io_error(_e: io::Error) -> ExprInst { 0usize.atom_exi() }

/// Writing command (string or binary)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WriteCmd {
  WBytes(Binary),
  WStr(String),
  Flush,
}

impl IOCmd for WriteCmd {
  type Stream = Sink;
  type Handle = SharedHandle<Sink>;
  type Result = WriteResult;

  fn execute(
    self,
    stream: &mut Self::Stream,
    _cancel: Canceller,
  ) -> Self::Result {
    let result = match &self {
      Self::Flush => stream.flush(),
      Self::WStr(str) => write!(stream, "{}", str).map(|_| ()),
      Self::WBytes(bytes) => stream.write_all(bytes.0.as_ref()).map(|_| ()),
    };
    WriteResult { result, cmd: self }
  }
}

pub struct WriteResult {
  pub cmd: WriteCmd,
  pub result: io::Result<()>,
}
impl WriteResult {
  pub fn dispatch(self, succ: ExprInst, fail: ExprInst) -> Vec<ExprInst> {
    match self.result {
      Ok(_) => vec![succ],
      Err(e) => vec![call(fail, vec![wrap_io_error(e)]).wrap()],
    }
  }
}
