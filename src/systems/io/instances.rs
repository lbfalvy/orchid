use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;

use super::flow::IOCmd;
use crate::foreign::Atomic;
use crate::interpreted::ExprInst;
use crate::systems::codegen::call;
use crate::systems::scheduler::{Canceller, SharedHandle};
use crate::systems::stl::Binary;
use crate::Literal;

pub type Source = BufReader<Box<dyn Read + Send>>;
pub type Sink = Box<dyn Write + Send>;

pub type SourceHandle = SharedHandle<Source>;
pub type SinkHandle = SharedHandle<Sink>;

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
  type Handle = SourceHandle;

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
          SRead::All => stream.read_to_string(&mut buf),
          SRead::Line => stream.read_line(&mut buf),
        };
        ReadResult::RStr(sread, sresult.map(|_| buf))
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
      ReadResult::RBin(_, Err(e)) | ReadResult::RStr(_, Err(e)) =>
        vec![call(fail, vec![wrap_io_error(e)]).wrap()],
      ReadResult::RBin(_, Ok(bytes)) => {
        let arg = Binary(Arc::new(bytes)).atom_cls().wrap();
        vec![call(succ, vec![arg]).wrap()]
      },
      ReadResult::RStr(_, Ok(text)) =>
        vec![call(succ, vec![Literal::Str(text.into()).into()]).wrap()],
    }
  }
}

/// Placeholder function for an eventual conversion from [io::Error] to Orchid
/// data
fn wrap_io_error(_e: io::Error) -> ExprInst {
  Literal::Uint(0u64).into()
}

/// Writing command (string or binary)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WriteCmd {
  WBytes(Binary),
  WStr(String),
  Flush,
}

impl IOCmd for WriteCmd {
  type Stream = Sink;
  type Handle = SinkHandle;
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
