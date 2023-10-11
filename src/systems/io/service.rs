#[allow(unused)] // for doc
use std::io::{BufReader, Read, Write};

use itertools::Itertools;
use rust_embed::RustEmbed;
use trait_set::trait_set;

use super::bindings::io_bindings;
use super::flow::{IOCmd, IOCmdHandlePack};
use super::instances::{ReadCmd, Sink, Source, WriteCmd};
use crate::facade::{IntoSystem, System};
use crate::foreign::cps_box::{init_cps, CPSBox};
use crate::foreign::Atomic;
use crate::interpreter::HandlerTable;
use crate::pipeline::file_loader::embed_to_map;
use crate::sourcefile::{FileEntry, FileEntryKind, Import};
use crate::systems::codegen::call;
use crate::systems::scheduler::{SeqScheduler, SharedHandle};
use crate::Location;

/// A shared type for sinks and sources
pub enum Stream {
  /// A Source, aka. a BufReader
  Source(Source),
  /// A Sink, aka. a Writer
  Sink(Sink),
}

trait_set! {
  /// The table of default streams to be overlain on the I/O module, typicially
  /// stdin, stdout, stderr.
  pub trait StreamTable<'a> = IntoIterator<Item = (&'a str, Stream)>
}

#[derive(RustEmbed)]
#[folder = "src/systems/io"]
#[prefix = "system/"]
#[include = "*.orc"]
struct IOEmbed;

/// A streaming I/O service for interacting with Rust's [Write] and [Read]
/// traits.
pub struct Service<'a, ST: IntoIterator<Item = (&'a str, Stream)>> {
  scheduler: SeqScheduler,
  global_streams: ST,
}
impl<'a, ST: IntoIterator<Item = (&'a str, Stream)>> Service<'a, ST> {
  /// Construct a new instance of the service
  pub fn new(scheduler: SeqScheduler, global_streams: ST) -> Self {
    Self { scheduler, global_streams }
  }
}

impl<'a, ST: IntoIterator<Item = (&'a str, Stream)>> IntoSystem<'static>
  for Service<'a, ST>
{
  fn into_system(self, i: &crate::Interner) -> crate::facade::System<'static> {
    let scheduler = self.scheduler.clone();
    let mut handlers = HandlerTable::new();
    handlers.register(move |cps: Box<CPSBox<IOCmdHandlePack<ReadCmd>>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, tail) = cps.unpack3();
      let fail1 = fail.clone();
      let result = scheduler.schedule(
        handle,
        move |mut stream, cancel| {
          let ret = cmd.execute(&mut stream, cancel);
          (stream, ret)
        },
        move |stream, res, _cancel| (stream, res.dispatch(succ, fail1)),
        |stream| (stream, Vec::new()),
      );
      match result {
        Ok(cancel) => Ok(call(tail, [init_cps(1, cancel).wrap()]).wrap()),
        Err(e) => Ok(call(fail, [e.atom_exi()]).wrap()),
      }
    });
    let scheduler = self.scheduler.clone();
    handlers.register(move |cps: Box<CPSBox<IOCmdHandlePack<WriteCmd>>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, tail) = cps.unpack3();
      let (succ1, fail1) = (succ, fail.clone());
      let result = scheduler.schedule(
        handle,
        move |mut stream, cancel| {
          let ret = cmd.execute(&mut stream, cancel);
          (stream, ret)
        },
        move |stream, res, _cancel| (stream, res.dispatch(succ1, fail1)),
        |stream| (stream, Vec::new()),
      );
      match result {
        Ok(cancel) => Ok(call(tail, [init_cps(1, cancel).wrap()]).wrap()),
        Err(e) => Ok(call(fail, [e.atom_exi()]).wrap()),
      }
    });
    let streams = self.global_streams.into_iter().map(|(n, stream)| {
      let handle = match stream {
        Stream::Sink(sink) =>
          Box::new(SharedHandle::wrap(sink)) as Box<dyn Atomic>,
        Stream::Source(source) => Box::new(SharedHandle::wrap(source)),
      };
      (n, handle)
    });
    System {
      handlers,
      name: ["system", "io"].into_iter().map_into().collect(),
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
      lexer_plugin: None,
      line_parser: None,
    }
  }
}
