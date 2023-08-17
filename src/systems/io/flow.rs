use std::collections::VecDeque;
use std::fmt::Display;

use hashbrown::HashMap;

use crate::foreign::ExternError;
use crate::systems::asynch::MessagePort;
use crate::utils::{take_with_output, Task};
use crate::ThreadPool;

pub trait StreamHandle: Clone + Send {
  fn new(id: usize) -> Self;
  fn id(&self) -> usize;
}

pub trait IOHandler<Cmd: IOCmd> {
  type Product;

  fn handle(self, result: Cmd::Result) -> Self::Product;
}

pub trait IOResult: Send {
  type Handler;
  type HandlerProduct;

  fn handle(self, handler: Self::Handler) -> Self::HandlerProduct;
}

pub struct IOEvent<Cmd: IOCmd> {
  pub result: Cmd::Result,
  pub stream: Cmd::Stream,
  pub handle: Cmd::Handle,
}

pub trait IOCmd: Send {
  type Stream: Send;
  type Result: Send;
  type Handle: StreamHandle;

  fn execute(self, stream: &mut Self::Stream) -> Self::Result;
}

pub struct IOTask<P: MessagePort, Cmd: IOCmd> {
  pub cmd: Cmd,
  pub stream: Cmd::Stream,
  pub handle: Cmd::Handle,
  pub port: P,
}

impl<P: MessagePort, Cmd: IOCmd + 'static> Task for IOTask<P, Cmd> {
  fn run(self) {
    let Self { cmd, handle, mut port, mut stream } = self;
    let result = cmd.execute(&mut stream);
    port.send(IOEvent::<Cmd> { handle, result, stream })
  }
}

#[derive(Debug, Clone)]
pub struct IOCmdHandlePack<Cmd: IOCmd> {
  pub cmd: Cmd,
  pub handle: Cmd::Handle,
}

enum StreamState<Cmd: IOCmd, H: IOHandler<Cmd>> {
  Free(Cmd::Stream),
  Busy { handler: H, queue: VecDeque<(Cmd, H)>, closing: bool },
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct NoActiveStream(usize);
impl ExternError for NoActiveStream {}
impl Display for NoActiveStream {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "The stream {} had already been closed", self.0)
  }
}

pub struct IOManager<P: MessagePort, Cmd: IOCmd + 'static, H: IOHandler<Cmd>> {
  next_id: usize,
  streams: HashMap<usize, StreamState<Cmd, H>>,
  on_close: Option<Box<dyn FnMut(Cmd::Stream)>>,
  thread_pool: ThreadPool<IOTask<P, Cmd>>,
  port: P,
}

impl<P: MessagePort, Cmd: IOCmd, H: IOHandler<Cmd>> IOManager<P, Cmd, H> {
  pub fn new(port: P, on_close: Option<Box<dyn FnMut(Cmd::Stream)>>) -> Self {
    Self {
      next_id: 0,
      streams: HashMap::new(),
      thread_pool: ThreadPool::new(),
      on_close,
      port,
    }
  }

  pub fn add_stream(&mut self, stream: Cmd::Stream) -> Cmd::Handle {
    let id = self.next_id;
    self.next_id += 1;
    self.streams.insert(id, StreamState::Free(stream));
    Cmd::Handle::new(id)
  }

  fn dispose_stream(&mut self, stream: Cmd::Stream) {
    match &mut self.on_close {
      Some(f) => f(stream),
      None => drop(stream),
    }
  }

  pub fn close_stream(
    &mut self,
    handle: Cmd::Handle,
  ) -> Result<(), NoActiveStream> {
    let state =
      (self.streams.remove(&handle.id())).ok_or(NoActiveStream(handle.id()))?;
    match state {
      StreamState::Free(stream) => self.dispose_stream(stream),
      StreamState::Busy { handler, queue, closing } => {
        let new_state = StreamState::Busy { handler, queue, closing: true };
        self.streams.insert(handle.id(), new_state);
        if closing {
          return Err(NoActiveStream(handle.id()));
        }
      },
    }
    Ok(())
  }

  pub fn command(
    &mut self,
    handle: Cmd::Handle,
    cmd: Cmd,
    new_handler: H,
  ) -> Result<(), NoActiveStream> {
    let state_mut = (self.streams.get_mut(&handle.id()))
      .ok_or(NoActiveStream(handle.id()))?;
    take_with_output(state_mut, |state| match state {
      StreamState::Busy { closing: true, .. } =>
        (state, Err(NoActiveStream(handle.id()))),
      StreamState::Busy { handler, mut queue, closing: false } => {
        queue.push_back((cmd, new_handler));
        (StreamState::Busy { handler, queue, closing: false }, Ok(()))
      },
      StreamState::Free(stream) => {
        let task = IOTask { cmd, stream, handle, port: self.port.clone() };
        self.thread_pool.submit(task);
        let new_state = StreamState::Busy {
          handler: new_handler,
          queue: VecDeque::new(),
          closing: false,
        };
        (new_state, Ok(()))
      },
    })
  }

  pub fn dispatch(&mut self, event: IOEvent<Cmd>) -> Option<H::Product> {
    let IOEvent { handle, result, stream } = event;
    let id = handle.id();
    let state =
      (self.streams.remove(&id)).expect("Event dispatched on unknown stream");
    let (handler, mut queue, closing) = match state {
      StreamState::Busy { handler, queue, closing } =>
        (handler, queue, closing),
      _ => panic!("Event dispatched but the source isn't locked"),
    };
    if let Some((cmd, handler)) = queue.pop_front() {
      let port = self.port.clone();
      self.thread_pool.submit(IOTask { handle, stream, cmd, port });
      self.streams.insert(id, StreamState::Busy { handler, queue, closing });
    } else if closing {
      self.dispose_stream(stream)
    } else {
      self.streams.insert(id, StreamState::Free(stream));
    };
    Some(handler.handle(result))
  }
}
