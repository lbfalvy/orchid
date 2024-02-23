//! Abstract implementation of the poller

use std::collections::BinaryHeap;
use std::mem;
use std::sync::mpsc::{channel, Receiver, RecvError, RecvTimeoutError, Sender};
use std::thread::sleep;
use std::time::{Duration, Instant};

use super::delete_cell::DeleteCell;

enum TimerKind<TOnce, TRec> {
  Once(DeleteCell<TOnce>),
  Recurring { period: Duration, data_cell: DeleteCell<TRec> },
}
impl<TOnce, TRec> Clone for TimerKind<TOnce, TRec> {
  fn clone(&self) -> Self {
    match self {
      Self::Once(c) => Self::Once(c.clone()),
      Self::Recurring { period, data_cell: data } =>
        Self::Recurring { period: *period, data_cell: data.clone() },
    }
  }
}

/// Indicates a bit of code which is to be executed at a
/// specific point in time
///
/// In order to work with Rust's builtin [BinaryHeap] which is a max heap, the
/// [Ord] implemenetation of this struct is reversed; it can be intuitively
/// thought of as ordering by urgency.
struct Timer<TOnce, TRec> {
  expires: Instant,
  kind: TimerKind<TOnce, TRec>,
}
impl<TOnce, TRec> Clone for Timer<TOnce, TRec> {
  fn clone(&self) -> Self { Self { expires: self.expires, kind: self.kind.clone() } }
}
impl<TOnce, TRec> Eq for Timer<TOnce, TRec> {}
impl<TOnce, TRec> PartialEq for Timer<TOnce, TRec> {
  fn eq(&self, other: &Self) -> bool { self.expires.eq(&other.expires) }
}
impl<TOnce, TRec> PartialOrd for Timer<TOnce, TRec> {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(other.cmp(self)) }
}
impl<TOnce, TRec> Ord for Timer<TOnce, TRec> {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering { other.expires.cmp(&self.expires) }
}

/// Representation of a scheduled timer
#[derive(Clone)]
pub struct TimerHandle<T>(DeleteCell<T>);
impl<T> TimerHandle<T> {
  /// Cancel the timer
  pub fn cancel(self) { mem::drop(self.0.take()) }
}

/// The abstract event poller implementation used by the standard asynch
/// subsystem.
pub struct Poller<TEv, TOnce, TRec: Clone> {
  timers: BinaryHeap<Timer<TOnce, TRec>>,
  receiver: Receiver<TEv>,
}

impl<TEv, TOnce, TRec: Clone> Poller<TEv, TOnce, TRec> {
  /// Create an event poller and a [Sender] that can produce events on it.
  pub fn new() -> (Sender<TEv>, Self) {
    let (sender, receiver) = channel();
    let this = Self { receiver, timers: BinaryHeap::new() };
    (sender, this)
  }

  /// Set a single-fire timer
  pub fn set_timeout(&mut self, duration: Duration, data: TOnce) -> TimerHandle<TOnce> {
    let data_cell = DeleteCell::new(data);
    self
      .timers
      .push(Timer { kind: TimerKind::Once(data_cell.clone()), expires: Instant::now() + duration });
    TimerHandle(data_cell)
  }

  /// Set a recurring timer
  pub fn set_interval(&mut self, period: Duration, data: TRec) -> TimerHandle<TRec> {
    let data_cell = DeleteCell::new(data);
    self.timers.push(Timer {
      expires: Instant::now() + period,
      kind: TimerKind::Recurring { period, data_cell: data_cell.clone() },
    });
    TimerHandle(data_cell)
  }

  /// Process a timer popped from the timers heap of this event loop.
  /// This function returns [None] if the timer had been cancelled. **This
  /// behaviour is different from [EventLoop::run] which is returns None if
  /// the event loop is empty, even though the types are compatible.**
  fn process_next_timer(
    &mut self,
    Timer { expires, kind }: Timer<TOnce, TRec>,
  ) -> Option<PollEvent<TEv, TOnce, TRec>> {
    Some(match kind {
      TimerKind::Once(data) => PollEvent::Once(data.take()?),
      TimerKind::Recurring { period, data_cell } => {
        let data = data_cell.clone_out()?;
        self.timers.push(Timer {
          expires: expires + period,
          kind: TimerKind::Recurring { period, data_cell },
        });
        PollEvent::Recurring(data)
      },
    })
  }

  /// Block until a message is received or the first timer expires
  pub fn run(&mut self) -> Option<PollEvent<TEv, TOnce, TRec>> {
    loop {
      if let Some(expires) = self.timers.peek().map(|t| t.expires) {
        return match self.receiver.recv_timeout(expires - Instant::now()) {
          Ok(t) => Some(PollEvent::Event(t)),
          Err(e) => {
            if e == RecvTimeoutError::Disconnected {
              // The receiver is now inert, but the timer must finish
              sleep(expires - Instant::now());
            }
            // pop and process the timer we've been waiting on
            let timer = self.timers.pop().expect("checked before wait");
            let result = self.process_next_timer(timer);
            // if the timer had been cancelled, repeat
            if result.is_none() {
              continue;
            }
            result
          },
        };
      } else {
        return match self.receiver.recv() {
          Ok(t) => Some(PollEvent::Event(t)),
          Err(RecvError) => None,
        };
      }
    }
  }
}

/// Events produced by [Poller].
pub enum PollEvent<TEv, TOnce, TRec> {
  /// An event was sent to the [Sender] associated with the [Poller].
  Event(TEv),
  /// A single-fire timer expired
  Once(TOnce),
  /// A recurring event fired
  Recurring(TRec),
}
