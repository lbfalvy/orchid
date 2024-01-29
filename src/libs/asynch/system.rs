//! Object to pass to [crate::facade::loader::Loader::add_system] to enable the
//! I/O subsystem. Also many other systems depend on it, these take a mut ref to
//! register themselves.

use std::any::{type_name, Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hashbrown::HashMap;
use ordered_float::NotNan;
use rust_embed::RustEmbed;

use super::poller::{PollEvent, Poller, TimerHandle};
use crate::facade::system::{IntoSystem, System};
use crate::foreign::atom::Atomic;
use crate::foreign::cps_box::CPSBox;
use crate::foreign::error::ExternError;
use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::handler::HandlerTable;
use crate::interpreter::nort::Expr;
use crate::libs::std::number::Numeric;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::utils::unwrap_or::unwrap_or;
use crate::virt_fs::{DeclTree, EmbeddedFS, PrefixFS, VirtFS};

#[derive(Debug, Clone)]
struct Timer {
  recurring: bool,
  delay: NotNan<f64>,
}

fn set_timer(rec: Inert<bool>, delay: Numeric) -> CPSBox<Timer> {
  CPSBox::new(2, Timer { recurring: rec.0, delay: delay.as_float() })
}

#[derive(Clone)]
struct CancelTimer(Arc<Mutex<dyn Fn() + Send>>);
impl CancelTimer {
  pub fn new<T: Send + Clone + 'static>(canceller: TimerHandle<T>) -> Self {
    Self(Arc::new(Mutex::new(move || canceller.clone().cancel())))
  }
  pub fn cancel(&self) { self.0.lock().unwrap()() }
}
impl Debug for CancelTimer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CancelTimer").finish_non_exhaustive()
  }
}

#[derive(Clone, Debug)]
struct Yield;
impl InertPayload for Yield {
  const TYPE_STR: &'static str = "asynch::yield";
}

/// Error indicating a yield command when all event producers and timers had
/// exited
#[derive(Clone)]
pub struct InfiniteBlock;
impl ExternError for InfiniteBlock {}
impl Display for InfiniteBlock {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    static MSG: &str = "User code yielded, but there are no timers or event \
                        producers to wake it up in the future";
    write!(f, "{}", MSG)
  }
}

/// A thread-safe handle that can be used to send events of any type
#[derive(Clone)]
pub struct MessagePort(Sender<Box<dyn Any + Send>>);
impl MessagePort {
  /// Send an event. Any type is accepted, handlers are dispatched by type ID
  pub fn send<T: Send + 'static>(&mut self, message: T) { let _ = self.0.send(Box::new(message)); }
}

fn gen() -> CodeGenInfo { CodeGenInfo::no_details("asynch") }

#[derive(RustEmbed)]
#[folder = "src/libs/asynch"]
#[include = "*.orc"]
struct AsynchEmbed;

fn code() -> DeclTree {
  DeclTree::ns("system::async", [DeclTree::leaf(
    PrefixFS::new(EmbeddedFS::new::<AsynchEmbed>(".orc", gen()), "", "io").rc(),
  )])
}

type AnyHandler<'a> = Box<dyn FnMut(Box<dyn Any>) -> Vec<Expr> + 'a>;

/// Datastructures the asynch system will eventually be constructed from.
pub struct AsynchSystem<'a> {
  poller: Poller<Box<dyn Any + Send>, Expr, Expr>,
  sender: Sender<Box<dyn Any + Send>>,
  handlers: HashMap<TypeId, AnyHandler<'a>>,
}

impl<'a> AsynchSystem<'a> {
  /// Create a new async event loop that allows registering handlers and taking
  /// references to the port before it's converted into a [System]
  #[must_use]
  pub fn new() -> Self {
    let (sender, poller) = Poller::new();
    Self { poller, sender, handlers: HashMap::new() }
  }

  /// Register a callback to be called on the owning thread when an object of
  /// the given type is found on the queue. Each type should signify a single
  /// command so each type should have exactly one handler.
  ///
  /// # Panics
  ///
  /// if the given type is already handled.
  pub fn register<T: 'static>(&mut self, mut f: impl FnMut(Box<T>) -> Vec<Expr> + 'a) {
    let cb = move |a: Box<dyn Any>| f(a.downcast().expect("keyed by TypeId"));
    let prev = self.handlers.insert(TypeId::of::<T>(), Box::new(cb));
    assert!(prev.is_none(), "Duplicate handlers for async event {}", type_name::<T>())
  }

  /// Obtain a message port for sending messages to the main thread. If an
  /// object is passed to the MessagePort that does not have a handler, the
  /// main thread panics.
  #[must_use]
  pub fn get_port(&self) -> MessagePort { MessagePort(self.sender.clone()) }
}

impl<'a> Default for AsynchSystem<'a> {
  fn default() -> Self { Self::new() }
}

impl<'a> IntoSystem<'a> for AsynchSystem<'a> {
  fn into_system(self) -> System<'a> {
    let Self { mut handlers, poller, .. } = self;
    let mut handler_table = HandlerTable::new();
    let polly = Rc::new(RefCell::new(poller));
    handler_table.register({
      let polly = polly.clone();
      move |t: &CPSBox<Timer>| {
        let mut polly = polly.borrow_mut();
        let (Timer { delay, recurring }, action, cont) = t.unpack2();
        let duration = Duration::from_secs_f64(**delay);
        let cancel_timer = match *recurring {
          true => CancelTimer::new(polly.set_interval(duration, action)),
          false => CancelTimer::new(polly.set_timeout(duration, action)),
        };
        let tpl = tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel_timer)));
        tpl.template(nort_gen(cont.location()), [cont])
      }
    });
    handler_table.register(move |t: &CPSBox<CancelTimer>| {
      let (command, cont) = t.unpack1();
      command.cancel();
      cont
    });
    handler_table.register({
      let polly = polly.clone();
      let mut microtasks = VecDeque::new();
      move |_: &Inert<Yield>| {
        if let Some(expr) = microtasks.pop_front() {
          return Ok(expr);
        }
        let mut polly = polly.borrow_mut();
        loop {
          let next = unwrap_or!(polly.run();
            return Err(InfiniteBlock.rc())
          );
          match next {
            PollEvent::Once(expr) => return Ok(expr),
            PollEvent::Recurring(expr) => return Ok(expr),
            PollEvent::Event(ev) => {
              let handler = (handlers.get_mut(&ev.as_ref().type_id()))
                .unwrap_or_else(|| panic!("Unhandled messgae type: {:?}", (*ev).type_id()));
              let events = handler(ev);
              // we got new microtasks
              if !events.is_empty() {
                microtasks = VecDeque::from(events);
                // trampoline
                let loc = CodeLocation::Gen(CodeGenInfo::no_details("system::asynch"));
                return Ok(Inert(Yield).atom_expr(loc));
              }
            },
          }
        }
      }
    });
    System {
      name: "system::asynch",
      lexer_plugins: vec![],
      line_parsers: vec![],
      constants: ConstTree::ns("system::async", [ConstTree::tree([
        xfn_ent("set_timer", [set_timer]),
        atom_ent("yield", [Inert(Yield)]),
      ])]),
      code: code(),
      prelude: Vec::new(),
      handlers: handler_table,
    }
  }
}
