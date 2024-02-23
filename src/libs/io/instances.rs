use std::io::{self, BufRead, Read, Write};
use std::sync::Arc;

use super::flow::IOCmd;
use super::service::{Sink, Source};
use crate::foreign::inert::Inert;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::nort::Expr;
use crate::libs::scheduler::cancel_flag::CancelFlag;
use crate::libs::scheduler::system::SharedHandle;
use crate::libs::std::binary::Binary;
use crate::libs::std::string::OrcString;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::sym;

/// String reading command
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(super) enum SRead {
  All,
  Line,
}

/// Binary reading command
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(super) enum BRead {
  All,
  N(usize),
  Until(u8),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(super) enum ReadCmd {
  RBytes(BRead),
  RStr(SRead),
}

impl IOCmd for ReadCmd {
  type Stream = Source;
  type Result = ReadResult;
  type Handle = SharedHandle<Source>;

  // This is a buggy rule, check manually
  #[allow(clippy::read_zero_byte_vec)]
  fn execute(self, stream: &mut Self::Stream, _cancel: CancelFlag) -> Self::Result {
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
pub(super) enum ReadResult {
  RStr(SRead, io::Result<String>),
  RBin(BRead, io::Result<Vec<u8>>),
}
impl ReadResult {
  pub fn dispatch(self, succ: Expr, fail: Expr) -> Vec<Expr> {
    vec![match self {
      ReadResult::RBin(_, Err(e)) | ReadResult::RStr(_, Err(e)) => io_error_handler(e, fail),
      ReadResult::RBin(_, Ok(bytes)) => tpl::A(tpl::Slot, tpl::V(Inert(Binary(Arc::new(bytes)))))
        .template(nort_gen(succ.location()), [succ]),
      ReadResult::RStr(_, Ok(text)) => tpl::A(tpl::Slot, tpl::V(Inert(OrcString::from(text))))
        .template(nort_gen(succ.location()), [succ]),
    }]
  }
}

/// Function to convert [io::Error] to Orchid data
pub(crate) fn io_error_handler(_e: io::Error, handler: Expr) -> Expr {
  let ctx = nort_gen(CodeLocation::new_gen(CodeGenInfo::no_details(sym!(system::io::io_error))));
  tpl::A(tpl::Slot, tpl::V(Inert(0usize))).template(ctx, [handler])
}

/// Writing command (string or binary)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum WriteCmd {
  WBytes(Binary),
  WStr(String),
  Flush,
}

impl IOCmd for WriteCmd {
  type Stream = Sink;
  type Handle = SharedHandle<Sink>;
  type Result = WriteResult;

  fn execute(self, stream: &mut Self::Stream, _cancel: CancelFlag) -> Self::Result {
    let result = match &self {
      Self::Flush => stream.flush(),
      Self::WStr(str) => write!(stream, "{}", str).map(|_| ()),
      Self::WBytes(bytes) => stream.write_all(bytes.0.as_ref()).map(|_| ()),
    };
    WriteResult { result, cmd: self }
  }
}

pub(super) struct WriteResult {
  #[allow(unused)]
  pub cmd: WriteCmd,
  pub result: io::Result<()>,
}
impl WriteResult {
  pub fn dispatch(self, succ: Expr, fail: Expr) -> Vec<Expr> {
    vec![self.result.map_or_else(|e| io_error_handler(e, fail), |()| succ)]
  }
}
