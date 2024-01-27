//! Object to pass to [crate::facade::loader::Loader::add_system] to enable the
//! I/O subsystem

use std::io::{BufReader, Read, Write};

use rust_embed::RustEmbed;
use trait_set::trait_set;

use super::bindings::io_bindings;
use super::flow::{IOCmd, IOCmdHandlePack};
use super::instances::{ReadCmd, WriteCmd};
use crate::facade::system::{IntoSystem, System};
use crate::foreign::cps_box::CPSBox;
use crate::foreign::inert::Inert;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::leaf;
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::handler::HandlerTable;
use crate::libs::scheduler::system::{SeqScheduler, SharedHandle};
use crate::location::CodeGenInfo;
use crate::name::VName;
use crate::pipeline::load_solution::Prelude;
use crate::virt_fs::{DeclTree, EmbeddedFS, PrefixFS, VirtFS};

/// Any type that we can read controlled amounts of data from
pub type Source = BufReader<Box<dyn Read + Send>>;
/// Any type that we can write data to
pub type Sink = Box<dyn Write + Send>;

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
  pub(super) trait StreamTable<'a> = IntoIterator<Item = (&'a str, Stream)>
}

fn gen() -> CodeGenInfo { CodeGenInfo::no_details("system::io") }

#[derive(RustEmbed)]
#[folder = "src/libs/io"]
#[include = "*.orc"]
struct IOEmbed;

fn code() -> DeclTree {
  DeclTree::ns("system::io", [DeclTree::leaf(
    PrefixFS::new(EmbeddedFS::new::<IOEmbed>(".orc", gen()), "", "io").rc(),
  )])
}

/// A streaming I/O service for interacting with Rust's [std::io::Write] and
/// [std::io::Read] traits.
pub struct IOService<'a, ST: IntoIterator<Item = (&'a str, Stream)>> {
  scheduler: SeqScheduler,
  global_streams: ST,
}
impl<'a, ST: IntoIterator<Item = (&'a str, Stream)>> IOService<'a, ST> {
  /// Construct a new instance of the service
  pub fn new(scheduler: SeqScheduler, global_streams: ST) -> Self {
    Self { scheduler, global_streams }
  }
}

impl<'a, ST: IntoIterator<Item = (&'a str, Stream)>> IntoSystem<'static>
  for IOService<'a, ST>
{
  fn into_system(self) -> System<'static> {
    let scheduler = self.scheduler.clone();
    let mut handlers = HandlerTable::new();
    handlers.register(move |cps: &CPSBox<IOCmdHandlePack<ReadCmd>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, cont) = cps.unpack3();
      let (cmd, fail1) = (*cmd, fail.clone());
      let result = scheduler.schedule(
        handle.clone(),
        move |mut stream, cancel| {
          let ret = cmd.execute(&mut stream, cancel);
          (stream, ret)
        },
        move |stream, res, _cancel| (stream, res.dispatch(succ, fail1)),
        |stream| (stream, Vec::new()),
      );
      match result {
        Ok(cancel) => tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel)))
          .template(nort_gen(cont.location()), [cont]),
        Err(e) => tpl::A(tpl::Slot, tpl::V(Inert(e)))
          .template(nort_gen(fail.location()), [fail]),
      }
    });
    let scheduler = self.scheduler.clone();
    handlers.register(move |cps: &CPSBox<IOCmdHandlePack<WriteCmd>>| {
      let (IOCmdHandlePack { cmd, handle }, succ, fail, cont) = cps.unpack3();
      let (succ1, fail1, cmd) = (succ, fail.clone(), cmd.clone());
      let result = scheduler.schedule(
        handle.clone(),
        move |mut stream, cancel| {
          let ret = cmd.execute(&mut stream, cancel);
          (stream, ret)
        },
        move |stream, res, _cancel| (stream, res.dispatch(succ1, fail1)),
        |stream| (stream, Vec::new()),
      );
      match result {
        Ok(cancel) => tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel)))
          .template(nort_gen(cont.location()), [cont]),
        Err(e) => tpl::A(tpl::Slot, tpl::V(Inert(e)))
          .template(nort_gen(fail.location()), [fail]),
      }
    });
    let streams = self.global_streams.into_iter().map(|(n, stream)| {
      let handle = match stream {
        Stream::Sink(sink) => leaf(tpl::V(Inert(SharedHandle::wrap(sink)))),
        Stream::Source(source) =>
          leaf(tpl::V(Inert(SharedHandle::wrap(source)))),
      };
      (n, handle)
    });
    System {
      handlers,
      name: "system::io",
      constants: io_bindings(streams),
      code: code(),
      prelude: vec![Prelude {
        target: VName::literal("system::io::prelude"),
        exclude: VName::literal("system::io"),
        owner: gen(),
      }],
      lexer_plugins: vec![],
      line_parsers: vec![],
    }
  }
}
