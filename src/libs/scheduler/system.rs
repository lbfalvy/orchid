//! Object to pass to [crate::facade::loader::Loader::add_system] to enable the
//! scheduling subsystem. Other systems also take clones as dependencies.
//!
//! ```
//! use orchidlang::facade::loader::Loader;
//! use orchidlang::libs::asynch::system::AsynchSystem;
//! use orchidlang::libs::scheduler::system::SeqScheduler;
//! use orchidlang::libs::std::std_system::StdConfig;
//!
//! let mut asynch = AsynchSystem::new();
//! let scheduler = SeqScheduler::new(&mut asynch);
//! let env = Loader::new()
//!   .add_system(StdConfig { impure: false })
//!   .add_system(asynch)
//!   .add_system(scheduler.clone());
//! ```

use std::any::{type_name, Any};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use trait_set::trait_set;

use super::busy::{BusyState, HandlerRes, NextItemReportKind, SyncOperation};
use super::cancel_flag::CancelFlag;
use super::id_map::IdMap;
use super::thread_pool::ThreadPool;
use crate::facade::system::{IntoSystem, System};
use crate::foreign::cps_box::CPSBox;
use crate::foreign::error::{AssertionError, RTResult};
use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::handler::HandlerTable;
use crate::interpreter::nort::Expr;
use crate::libs::asynch::system::{AsynchSystem, MessagePort};
use crate::utils::ddispatch::Request;
use crate::utils::take_with_output::take_with_output;
use crate::utils::unwrap_or::unwrap_or;
use crate::virt_fs::DeclTree;

pub(super) enum SharedResource<T> {
  Free(T),
  Busy(BusyState<T>),
  Taken,
}

/// Possible states of a shared resource
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum SharedState {
  /// The resource is ready to be used or taken
  Free,
  /// The resource is currently in use but operations can be asynchronously
  /// scheduled on it
  Busy,
  /// The resource is currently in use and a consuming seal has already been
  /// scheduled, therefore further operations cannot access it and it will
  /// transition to [SharedState::Taken] as soon as the currently pending
  /// operations finish or are cancelled.
  Sealed,
  /// The resource has been removed from this location.
  Taken,
}

/// A shared handle for a resource of type `T` that can be used with a
/// [SeqScheduler] to execute mutating operations one by one in worker threads.
pub struct SharedHandle<T>(pub(super) Arc<Mutex<SharedResource<T>>>);

impl<T> SharedHandle<T> {
  /// Wrap a value to be accessible to a [SeqScheduler].
  pub fn wrap(t: T) -> Self { Self(Arc::new(Mutex::new(SharedResource::Free(t)))) }

  /// Check the state of the handle
  pub fn state(&self) -> SharedState {
    match &*self.0.lock().unwrap() {
      SharedResource::Busy(b) if b.is_sealed() => SharedState::Sealed,
      SharedResource::Busy(_) => SharedState::Busy,
      SharedResource::Free(_) => SharedState::Free,
      SharedResource::Taken => SharedState::Taken,
    }
  }

  /// Remove the value from the handle if it's free. To interact with a handle
  /// you probably want to use a [SeqScheduler], but sometimes this makes
  /// sense as eg. an optimization. You can return the value after processing
  /// via [SharedHandle::untake].
  pub fn take(&self) -> Option<T> {
    take_with_output(&mut *self.0.lock().unwrap(), |state| match state {
      SharedResource::Free(t) => (SharedResource::Taken, Some(t)),
      _ => (state, None),
    })
  }

  /// Return the value to a handle that doesn't have one. The intended use case
  /// is to return values synchronously after they have been removed with
  /// [SharedHandle::take].
  pub fn untake(&self, value: T) -> Result<(), T> {
    take_with_output(&mut *self.0.lock().unwrap(), |state| match state {
      SharedResource::Taken => (SharedResource::Free(value), Ok(())),
      _ => (state, Err(value)),
    })
  }
}
impl<T> Clone for SharedHandle<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}
impl<T> fmt::Debug for SharedHandle<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("SharedHandle")
      .field("state", &self.state())
      .field("type", &type_name::<T>())
      .finish()
  }
}
impl<T: Send + 'static> InertPayload for SharedHandle<T> {
  const TYPE_STR: &'static str = "a SharedHandle";
  fn respond(&self, mut request: Request) {
    request.serve_with(|| {
      let this = self.clone();
      TakeCmd(Arc::new(move |sch| {
        let _ = sch.seal(this.clone(), |_| Vec::new());
      }))
    })
  }
}

#[derive(Clone)]
struct TakeCmd(pub Arc<dyn Fn(SeqScheduler) + Send + Sync>);
impl fmt::Debug for TakeCmd {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "A command to drop a shared resource")
  }
}

/// Error produced when an operation is scheduled or a seal placed on a resource
/// which is either already sealed or taken.
#[derive(Debug, Clone)]
pub struct SealedOrTaken;
impl InertPayload for SealedOrTaken {
  const TYPE_STR: &'static str = "SealedOrTaken";
}

fn take_and_drop(x: Expr) -> RTResult<CPSBox<TakeCmd>> {
  match x.clause.request() {
    Some(t) => Ok(CPSBox::<TakeCmd>::new(1, t)),
    None => AssertionError::fail(x.location(), "SharedHandle", format!("{x}")),
  }
}

fn is_taken_e(x: Expr) -> Inert<bool> { Inert(x.downcast::<Inert<SealedOrTaken>>().is_ok()) }

trait_set! {
  /// The part of processing a blocking I/O task that cannot be done on a remote
  /// thread, eg. because it accesses other systems or Orchid code.
  trait NonSendFn = FnOnce(Box<dyn Any + Send>, SeqScheduler) -> Vec<Expr>;
}

struct SyncReply {
  opid: u64,
  data: Box<dyn Any + Send>,
}

struct CheshireCat {
  pool: ThreadPool<Box<dyn FnOnce() + Send>>,
  pending: RefCell<IdMap<Box<dyn NonSendFn>>>,
  port: MessagePort,
}

/// A task scheduler that executes long blocking operations that have mutable
/// access to a shared one by one on a worker thread. The resources are
/// held in [SharedHandle]s
#[derive(Clone)]
pub struct SeqScheduler(Rc<CheshireCat>);
impl SeqScheduler {
  /// Creates a new [SeqScheduler]. The new object is also kept alive by a
  /// callback in the provided [AsynchSystem]. There should be at most one
  pub fn new(asynch: &mut AsynchSystem) -> Self {
    let this = Self(Rc::new(CheshireCat {
      pending: RefCell::new(IdMap::new()),
      pool: ThreadPool::new(),
      port: asynch.get_port(),
    }));
    let this1 = this.clone();
    // referenced by asynch, references this
    asynch.register(move |res: Box<SyncReply>| {
      let callback = this1.0.pending.borrow_mut().remove(res.opid).expect(
        "Received reply for task we didn't start. This likely means that \
         there are multiple SequencingContexts attached to the same \
         AsynchSystem.",
      );
      callback(res.data, this1.clone())
    });
    this
  }

  /// Submit an action to be executed on a worker thread which can own the data
  /// in the handle.
  ///
  /// * handle - data to be transformed
  /// * operation - long blocking mutation to execute off-thread.
  /// * handler - process the results, talk to other systems, generate and run
  ///   Orchid code.
  /// * early_cancel - clean up in case the task got cancelled before it was
  ///   scheduled. This is an optimization so that threads aren't spawned if a
  ///   large batch of tasks is scheduled and then cancelled.
  pub fn schedule<T: Send + 'static, U: Send + 'static>(
    &self,
    handle: SharedHandle<T>,
    operation: impl FnOnce(T, CancelFlag) -> (T, U) + Send + 'static,
    handler: impl FnOnce(T, U, CancelFlag) -> HandlerRes<T> + Send + 'static,
    early_cancel: impl FnOnce(T) -> HandlerRes<T> + Send + 'static,
  ) -> Result<CancelFlag, SealedOrTaken> {
    take_with_output(&mut *handle.0.lock().unwrap(), {
      let handle = handle.clone();
      |state| {
        match state {
          SharedResource::Taken => (SharedResource::Taken, Err(SealedOrTaken)),
          SharedResource::Busy(mut b) => match b.enqueue(operation, handler, early_cancel) {
            Some(cancelled) => (SharedResource::Busy(b), Ok(cancelled)),
            None => (SharedResource::Busy(b), Err(SealedOrTaken)),
          },
          SharedResource::Free(t) => {
            let cancelled = CancelFlag::new();
            drop(early_cancel); // cannot possibly be useful
            let op_erased: SyncOperation<T> = Box::new(|t, c| {
              let (t, u) = operation(t, c);
              (t, Box::new(u))
            });
            self.submit(t, handle, cancelled.clone(), op_erased);
            (SharedResource::Busy(BusyState::new(handler)), Ok(cancelled))
          },
        }
      }
    })
  }

  /// Run an operation asynchronously and then process its result in thread,
  /// without queuing on any particular data.
  pub fn run_orphan<T: Send + 'static>(
    &self,
    operation: impl FnOnce(CancelFlag) -> T + Send + 'static,
    handler: impl FnOnce(T, CancelFlag) -> Vec<Expr> + 'static,
  ) -> CancelFlag {
    let cancelled = CancelFlag::new();
    let canc1 = cancelled.clone();
    let opid = self.0.pending.borrow_mut().insert(Box::new(|data: Box<dyn Any + Send>, _| {
      handler(*data.downcast().expect("This is associated by ID"), canc1)
    }));
    let canc1 = cancelled.clone();
    let mut port = self.0.port.clone();
    self.0.pool.submit(Box::new(move || {
      port.send(SyncReply { opid, data: Box::new(operation(canc1)) });
    }));
    cancelled
  }

  /// Schedule a function that will consume the value. After this the handle is
  /// considered sealed and all [SeqScheduler::schedule] calls will fail.
  pub fn seal<T>(
    &self,
    handle: SharedHandle<T>,
    seal: impl FnOnce(T) -> Vec<Expr> + Sync + Send + 'static,
  ) -> Result<Vec<Expr>, SealedOrTaken> {
    take_with_output(&mut *handle.0.lock().unwrap(), |state| match state {
      SharedResource::Busy(mut b) if !b.is_sealed() => {
        b.seal(seal);
        (SharedResource::Busy(b), Ok(Vec::new()))
      },
      SharedResource::Busy(_) => (state, Err(SealedOrTaken)),
      SharedResource::Taken => (SharedResource::Taken, Err(SealedOrTaken)),
      SharedResource::Free(t) => (SharedResource::Taken, Ok(seal(t))),
    })
  }

  /// Asynchronously recursive function to schedule a new task for execution and
  /// act upon its completion. The self-reference is passed into the callback
  /// from the callback passed to the [AsynchSystem] so that if the task is
  /// never resolved but the [AsynchSystem] through which the resolving event
  /// would arrive is dropped this [SeqScheduler] is also dropped.
  fn submit<T: Send + 'static>(
    &self,
    t: T,
    handle: SharedHandle<T>,
    cancelled: CancelFlag,
    operation: SyncOperation<T>,
  ) {
    // referenced by self until run, references handle
    let opid = self.0.pending.borrow_mut().insert(Box::new({
      let cancelled = cancelled.clone();
      move |data: Box<dyn Any + Send>, this: SeqScheduler| {
        let (t, u): (T, Box<dyn Any + Send>) = *data.downcast().expect("This is associated by ID");
        let handle2 = handle.clone();
        take_with_output(&mut *handle.0.lock().unwrap(), |state| {
          let busy = unwrap_or! { state => SharedResource::Busy;
            panic!("Handle with outstanding invocation must be busy")
          };
          let report = busy.rotate(t, u, cancelled);
          match report.kind {
            NextItemReportKind::Free(t) => (SharedResource::Free(t), report.events),
            NextItemReportKind::Taken => (SharedResource::Taken, report.events),
            NextItemReportKind::Next { instance, cancelled, operation, rest } => {
              this.submit(instance, handle2, cancelled, operation);
              (SharedResource::Busy(rest), report.events)
            },
          }
        })
      }
    }));
    let mut port = self.0.port.clone();
    // referenced by thread until run, references port
    self.0.pool.submit(Box::new(move || {
      port.send(SyncReply { opid, data: Box::new(operation(t, cancelled)) })
    }))
  }
}

impl IntoSystem<'static> for SeqScheduler {
  fn into_system(self) -> System<'static> {
    let mut handlers = HandlerTable::new();
    handlers.register(|cmd: &CPSBox<CancelFlag>| {
      let (canceller, cont) = cmd.unpack1();
      canceller.cancel();
      cont
    });
    handlers.register(move |cmd: &CPSBox<TakeCmd>| {
      let (TakeCmd(cb), cont) = cmd.unpack1();
      cb(self.clone());
      cont
    });
    System {
      name: "system::scheduler",
      prelude: Vec::new(),
      code: DeclTree::empty(),
      handlers,
      lexer_plugins: vec![],
      line_parsers: vec![],
      constants: ConstTree::ns("system::scheduler", [ConstTree::tree([
        xfn_ent("is_taken_e", [is_taken_e]),
        xfn_ent("take_and_drop", [take_and_drop]),
      ])]),
    }
  }
}
