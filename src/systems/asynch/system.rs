use std::any::{type_name, Any, TypeId};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use hashbrown::HashMap;
use ordered_float::NotNan;
use rust_embed::RustEmbed;

use crate::facade::{IntoSystem, System};
use crate::foreign::cps_box::{init_cps, CPSBox};
use crate::foreign::{Atomic, ExternError, InertAtomic};
use crate::interpreted::ExprInst;
use crate::interpreter::HandlerTable;
use crate::pipeline::file_loader::embed_to_map;
use crate::systems::codegen::call;
use crate::systems::stl::Boolean;
use crate::utils::poller::{PollEvent, Poller};
use crate::utils::unwrap_or;
use crate::{define_fn, ConstTree, Interner};

#[derive(Debug, Clone)]
struct Timer {
  recurring: Boolean,
  duration: NotNan<f64>,
}
define_fn! {expr=x in
  SetTimer {
    recurring: Boolean,
    duration: NotNan<f64>
  } => Ok(init_cps(2, Timer{ recurring, duration }))
}

#[derive(Clone)]
struct CancelTimer(Rc<dyn Fn()>);
impl Debug for CancelTimer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "opaque cancel operation")
  }
}

#[derive(Clone, Debug)]
struct Yield;
impl InertAtomic for Yield {
  fn type_str() -> &'static str { "a yield command" }
}

/// Error indicating a yield command when all event producers and timers had
/// exited
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
  pub fn send<T: Send + 'static>(&mut self, message: T) {
    let _ = self.0.send(Box::new(message));
  }
}

#[derive(RustEmbed)]
#[folder = "src/systems/asynch"]
#[prefix = "system/"]
#[include = "*.orc"]
struct AsynchEmbed;

type AnyHandler<'a> = Box<dyn FnMut(Box<dyn Any>) -> Vec<ExprInst> + 'a>;

/// Datastructures the asynch system will eventually be constructed from.
pub struct AsynchSystem<'a> {
  poller: Poller<Box<dyn Any + Send>, ExprInst, ExprInst>,
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
  pub fn register<T: 'static>(
    &mut self,
    mut f: impl FnMut(Box<T>) -> Vec<ExprInst> + 'a,
  ) {
    let cb = move |a: Box<dyn Any>| f(a.downcast().expect("keyed by TypeId"));
    let prev = self.handlers.insert(TypeId::of::<T>(), Box::new(cb));
    assert!(
      prev.is_none(),
      "Duplicate handlers for async event {}",
      type_name::<T>()
    )
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
  fn into_system(self, i: &Interner) -> System<'a> {
    let Self { mut handlers, poller, .. } = self;
    let mut handler_table = HandlerTable::new();
    let polly = Rc::new(RefCell::new(poller));
    handler_table.register({
      let polly = polly.clone();
      move |t: Box<CPSBox<Timer>>| {
        let mut polly = polly.borrow_mut();
        let (timeout, action, cont) = t.unpack2();
        let duration = Duration::from_secs_f64(*timeout.duration);
        let cancel_timer = if timeout.recurring.0 {
          CancelTimer(Rc::new(polly.set_interval(duration, action)))
        } else {
          CancelTimer(Rc::new(polly.set_timeout(duration, action)))
        };
        Ok(call(cont, [init_cps(1, cancel_timer).wrap()]).wrap())
      }
    });
    handler_table.register(move |t: Box<CPSBox<CancelTimer>>| {
      let (command, cont) = t.unpack1();
      command.0.as_ref()();
      Ok(cont)
    });
    handler_table.register({
      let polly = polly.clone();
      let mut microtasks = VecDeque::new();
      move |_: Box<Yield>| {
        if let Some(expr) = microtasks.pop_front() {
          return Ok(expr);
        }
        let mut polly = polly.borrow_mut();
        loop {
          let next = unwrap_or!(polly.run();
            return Err(InfiniteBlock.into_extern())
          );
          match next {
            PollEvent::Once(expr) => return Ok(expr),
            PollEvent::Recurring(expr) => return Ok(expr),
            PollEvent::Event(ev) => {
              let handler = (handlers.get_mut(&ev.as_ref().type_id()))
                .unwrap_or_else(|| {
                  panic!("Unhandled messgae type: {:?}", ev.type_id())
                });
              let events = handler(ev);
              // we got new microtasks
              if !events.is_empty() {
                microtasks = VecDeque::from(events);
                // trampoline
                return Ok(Yield.atom_exi());
              }
            },
          }
        }
      }
    });
    System {
      name: vec!["system".to_string(), "asynch".to_string()],
      constants: ConstTree::namespace(
        [i.i("system"), i.i("async")],
        ConstTree::tree([
          (i.i("set_timer"), ConstTree::xfn(SetTimer)),
          (i.i("yield"), ConstTree::atom(Yield)),
        ]),
      )
      .unwrap_tree(),
      code: embed_to_map::<AsynchEmbed>(".orc", i),
      prelude: Vec::new(),
      handlers: handler_table,
    }
  }
}
