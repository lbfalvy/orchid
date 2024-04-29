use std::any::Any;
use std::collections::VecDeque;

use super::cancel_flag::CancelFlag;
use crate::interpreter::nort::Expr;

pub type SyncResult<T> = (T, Box<dyn Any + Send>);
/// Output from handlers contains the resource being processed and any Orchid
/// handlers executed as a result of the operation
pub type HandlerRes<T> = (T, Vec<Expr>);
pub type SyncOperation<T> = Box<dyn FnOnce(T, CancelFlag) -> SyncResult<T> + Send>;
pub type SyncOpResultHandler<T> =
  Box<dyn FnOnce(T, Box<dyn Any + Send>, CancelFlag) -> (T, Vec<Expr>) + Send>;

struct SyncQueueItem<T> {
  cancelled: CancelFlag,
  operation: SyncOperation<T>,
  handler: SyncOpResultHandler<T>,
  early_cancel: Box<dyn FnOnce(T) -> (T, Vec<Expr>) + Send>,
}

pub enum NextItemReportKind<T> {
  Free(T),
  Next { instance: T, cancelled: CancelFlag, operation: SyncOperation<T>, rest: BusyState<T> },
  Taken,
}

pub struct NextItemReport<T> {
  pub kind: NextItemReportKind<T>,
  pub events: Vec<Expr>,
}

pub(super) struct BusyState<T> {
  handler: SyncOpResultHandler<T>,
  queue: VecDeque<SyncQueueItem<T>>,
  seal: Option<Box<dyn FnOnce(T) -> Vec<Expr> + Send>>,
}
impl<T> BusyState<T> {
  pub fn new<U: 'static + Send>(
    handler: impl FnOnce(T, U, CancelFlag) -> HandlerRes<T> + Send + 'static,
  ) -> Self {
    BusyState {
      handler: Box::new(|t, payload, cancel| {
        let u = *payload.downcast().expect("mismatched initial handler and operation");
        handler(t, u, cancel)
      }),
      queue: VecDeque::new(),
      seal: None,
    }
  }

  /// Add a new operation to the queue. Returns Some if the operation was
  /// successfully enqueued and None if the queue is already sealed.
  pub fn enqueue<U: 'static + Send>(
    &mut self,
    operation: impl FnOnce(T, CancelFlag) -> (T, U) + Send + 'static,
    handler: impl FnOnce(T, U, CancelFlag) -> HandlerRes<T> + Send + 'static,
    early_cancel: impl FnOnce(T) -> HandlerRes<T> + Send + 'static,
  ) -> Option<CancelFlag> {
    if self.seal.is_some() {
      return None;
    }
    let cancelled = CancelFlag::new();
    self.queue.push_back(SyncQueueItem {
      cancelled: cancelled.clone(),
      early_cancel: Box::new(early_cancel),
      operation: Box::new(|t, c| {
        let (t, r) = operation(t, c);
        (t, Box::new(r))
      }),
      handler: Box::new(|t, u, c| {
        let u: Box<U> = u.downcast().expect("mismatched handler and operation");
        handler(t, *u, c)
      }),
    });
    Some(cancelled)
  }

  pub fn seal(&mut self, recipient: impl FnOnce(T) -> Vec<Expr> + Send + 'static) {
    assert!(self.seal.is_none(), "Already sealed");
    self.seal = Some(Box::new(recipient))
  }

  pub fn is_sealed(&self) -> bool { self.seal.is_some() }

  pub fn rotate(
    mut self,
    instance: T,
    result: Box<dyn Any + Send>,
    cancelled: CancelFlag,
  ) -> NextItemReport<T> {
    let (mut instance, mut events) = (self.handler)(instance, result, cancelled);
    let next_item = loop {
      if let Some(candidate) = self.queue.pop_front() {
        if candidate.cancelled.is_cancelled() {
          let ret = (candidate.early_cancel)(instance);
          instance = ret.0;
          events.extend(ret.1);
        } else {
          break candidate;
        }
      } else if let Some(seal) = self.seal.take() {
        seal(instance);
        let kind = NextItemReportKind::Taken;
        return NextItemReport { events, kind };
      } else {
        let kind = NextItemReportKind::Free(instance);
        return NextItemReport { events, kind };
      }
    };
    self.handler = next_item.handler;
    NextItemReport {
      events,
      kind: NextItemReportKind::Next {
        instance,
        cancelled: next_item.cancelled,
        operation: next_item.operation,
        rest: self,
      },
    }
  }
}
