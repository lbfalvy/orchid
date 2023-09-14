#![allow(non_upper_case_globals)] // RustEmbed is sloppy
use std::cell::RefCell;
use std::rc::Rc;

use rust_embed::RustEmbed;
use trait_set::trait_set;

use super::bindings::io_bindings;
use super::flow::{IOCmdHandlePack, IOManager, NoActiveStream};
use super::instances::{
  ReadCmd, ReadManager, Sink, SinkHandle, Source, SourceHandle, WriteCmd,
  WriteManager,
};
use crate::facade::{IntoSystem, System};
use crate::foreign::cps_box::CPSBox;
use crate::foreign::{Atomic, ExternError};
use crate::interpreter::HandlerTable;
use crate::pipeline::file_loader::embed_to_map;
use crate::sourcefile::{FileEntry, FileEntryKind, Import};
use crate::systems::asynch::AsynchSystem;
use crate::{Interner, Location};

trait_set! {
  pub trait StreamTable = IntoIterator<Item = (&'static str, IOStream)>
}

#[derive(RustEmbed)]
#[folder = "src/systems/io"]
#[prefix = "system/"]
#[include = "*.orc"]
struct IOEmbed;

/// A registry that stores IO streams and executes blocking operations on them
/// in a distinct thread pool
pub struct IOSystem<ST: StreamTable> {
  read_system: Rc<RefCell<ReadManager>>,
  write_system: Rc<RefCell<WriteManager>>,
  global_streams: ST,
}
impl<ST: StreamTable> IOSystem<ST> {
  fn new(
    asynch: &AsynchSystem,
    on_sink_close: Option<Box<dyn FnMut(Sink)>>,
    on_source_close: Option<Box<dyn FnMut(Source)>>,
    global_streams: ST,
  ) -> Self {
    Self {
      read_system: Rc::new(RefCell::new(IOManager::new(
        asynch.get_port(),
        on_source_close,
      ))),
      write_system: Rc::new(RefCell::new(IOManager::new(
        asynch.get_port(),
        on_sink_close,
      ))),
      global_streams,
    }
  }
  /// Register a new source so that it can be used with IO commands
  pub fn add_source(&self, source: Source) -> SourceHandle {
    self.read_system.borrow_mut().add_stream(source)
  }
  /// Register a new sink so that it can be used with IO operations
  pub fn add_sink(&self, sink: Sink) -> SinkHandle {
    self.write_system.borrow_mut().add_stream(sink)
  }
  /// Schedule a source to be closed when all currently enqueued IO operations
  /// finish.
  pub fn close_source(
    &self,
    handle: SourceHandle,
  ) -> Result<(), NoActiveStream> {
    self.read_system.borrow_mut().close_stream(handle)
  }
  /// Schedule a sink to be closed when all current IO operations finish.
  pub fn close_sink(&self, handle: SinkHandle) -> Result<(), NoActiveStream> {
    self.write_system.borrow_mut().close_stream(handle)
  }
}

/// A shared type for sinks and sources
pub enum IOStream {
  /// A Source, aka. a BufReader
  Source(Source),
  /// A Sink, aka. a Writer
  Sink(Sink),
}

/// Construct an [IOSystem]. An event loop ([AsynchConfig]) is required to
/// sequence IO events on the interpreter thread.
///
/// This is a distinct function because [IOSystem]
/// takes a generic parameter which is initialized from an existential in the
/// [AsynchConfig].
pub fn io_system(
  asynch: &'_ mut AsynchSystem,
  on_sink_close: Option<Box<dyn FnMut(Sink)>>,
  on_source_close: Option<Box<dyn FnMut(Source)>>,
  std_streams: impl IntoIterator<Item = (&'static str, IOStream)>,
) -> IOSystem<impl StreamTable> {
  let this = IOSystem::new(asynch, on_sink_close, on_source_close, std_streams);
  let (r, w) = (this.read_system.clone(), this.write_system.clone());
  asynch.register(move |event| vec![r.borrow_mut().dispatch(*event)]);
  asynch.register(move |event| vec![w.borrow_mut().dispatch(*event)]);
  this
}

impl<'a, ST: StreamTable + 'a> IntoSystem<'a> for IOSystem<ST> {
  fn into_system(self, i: &Interner) -> System<'a> {
    let (r, w) = (self.read_system.clone(), self.write_system.clone());
    let mut handlers = HandlerTable::new();
    handlers.register(move |cps: &CPSBox<IOCmdHandlePack<ReadCmd>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, tail) = cps.unpack3();
      (r.borrow_mut())
        .command(*handle, *cmd, (succ.clone(), fail.clone()))
        .map_err(|e| e.into_extern())?;
      Ok(tail.clone())
    });
    handlers.register(move |cps: &CPSBox<IOCmdHandlePack<WriteCmd>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, tail) = cps.unpack3();
      (w.borrow_mut())
        .command(*handle, cmd.clone(), (succ.clone(), fail.clone()))
        .map_err(|e| e.into_extern())?;
      Ok(tail.clone())
    });
    let streams = self.global_streams.into_iter().map(|(n, stream)| {
      let handle = match stream {
        IOStream::Sink(sink) =>
          Box::new(self.write_system.borrow_mut().add_stream(sink))
            as Box<dyn Atomic>,
        IOStream::Source(source) =>
          Box::new(self.read_system.borrow_mut().add_stream(source)),
      };
      (n, handle)
    });
    System {
      name: vec!["system".to_string(), "io".to_string()],
      constants: io_bindings(i, streams).unwrap_tree(),
      code: embed_to_map::<IOEmbed>(".orc", i),
      prelude: vec![FileEntry {
        locations: vec![Location::Unknown],
        kind: FileEntryKind::Import(vec![Import {
          location: Location::Unknown,
          path: vec![i.i("system"), i.i("io"), i.i("prelude")],
          name: None,
        }]),
      }],
      handlers,
    }
  }
}
