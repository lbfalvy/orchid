use std::fmt;

use crate::foreign::error::RTError;
use crate::libs::scheduler::cancel_flag::CancelFlag;

pub trait IOHandler<T> {
  type Product;

  fn handle(self, result: T) -> Self::Product;
  fn early_cancel(self) -> Self::Product;
}

pub trait IOResult: Send {
  type Handler;
  type HandlerProduct;

  fn handle(self, handler: Self::Handler) -> Self::HandlerProduct;
}

pub trait IOCmd: Send {
  type Stream: Send;
  type Result: Send;
  type Handle;

  fn execute(self, stream: &mut Self::Stream, cancel: CancelFlag) -> Self::Result;
}

#[derive(Debug, Clone)]
pub struct IOCmdHandlePack<Cmd: IOCmd> {
  pub cmd: Cmd,
  pub handle: Cmd::Handle,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct NoActiveStream(usize);
impl RTError for NoActiveStream {}
impl fmt::Display for NoActiveStream {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "The stream {} had already been closed", self.0)
  }
}
