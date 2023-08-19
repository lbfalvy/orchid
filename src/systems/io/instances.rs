use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;

use super::flow::{IOCmd, IOHandler, IOManager, StreamHandle};
use crate::foreign::Atomic;
use crate::interpreted::ExprInst;
use crate::systems::codegen::call;
use crate::systems::stl::Binary;
use crate::{atomic_inert, Literal};

pub type Source = BufReader<Box<dyn Read + Send>>;
pub type Sink = Box<dyn Write + Send>;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceHandle(usize);
atomic_inert!(SourceHandle, "an input stream handle");
impl StreamHandle for SourceHandle {
  fn new(id: usize) -> Self {
    Self(id)
  }
  fn id(&self) -> usize {
    self.0
  }
}
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SinkHandle(usize);
atomic_inert!(SinkHandle, "an output stream handle");
impl StreamHandle for SinkHandle {
  fn new(id: usize) -> Self {
    Self(id)
  }
  fn id(&self) -> usize {
    self.0
  }
}

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
  fn execute(self, stream: &mut Self::Stream) -> Self::Result {
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

impl IOHandler<ReadCmd> for (ExprInst, ExprInst) {
  type Product = ExprInst;

  fn handle(self, result: ReadResult) -> Self::Product {
    let (succ, fail) = self;
    match result {
      ReadResult::RBin(_, Err(e)) | ReadResult::RStr(_, Err(e)) =>
        call(fail, vec![wrap_io_error(e)]).wrap(),
      ReadResult::RBin(_, Ok(bytes)) =>
        call(succ, vec![Binary(Arc::new(bytes)).atom_cls().wrap()]).wrap(),
      ReadResult::RStr(_, Ok(text)) =>
        call(succ, vec![Literal::Str(text.into()).into()]).wrap(),
    }
  }
}

/// Placeholder function for an eventual conversion from [io::Error] to Orchid
/// data
fn wrap_io_error(_e: io::Error) -> ExprInst {
  Literal::Uint(0u64).into()
}

pub type ReadManager<P> = IOManager<P, ReadCmd, (ExprInst, ExprInst)>;

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

  fn execute(self, stream: &mut Self::Stream) -> Self::Result {
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
impl IOHandler<WriteCmd> for (ExprInst, ExprInst) {
  type Product = ExprInst;

  fn handle(self, result: WriteResult) -> Self::Product {
    let (succ, fail) = self;
    match result.result {
      Ok(_) => succ,
      Err(e) => call(fail, vec![wrap_io_error(e)]).wrap(),
    }
  }
}

pub type WriteManager<P> = IOManager<P, WriteCmd, (ExprInst, ExprInst)>;
