//! Multiple-listener-single-delivery event system.

use std::mem;
use std::sync::mpsc::{self, sync_channel};
use std::sync::Mutex;

struct Reply<T, U> {
  resub: bool,
  outcome: Result<U, T>,
}

struct Listener<T, E> {
  sink: mpsc::SyncSender<T>,
  source: mpsc::Receiver<Reply<T, E>>,
}

pub struct Event<T, U> {
  listeners: Mutex<Vec<Listener<T, U>>>,
}
impl<T, U> Event<T, U> {
  pub const fn new() -> Self { Self { listeners: Mutex::new(Vec::new()) } }

  pub fn dispatch(&self, mut ev: T) -> Option<U> {
    let mut listeners = self.listeners.lock().unwrap();
    let mut alt_list = Vec::with_capacity(listeners.len());
    mem::swap(&mut *listeners, &mut alt_list);
    let mut items = alt_list.into_iter();
    while let Some(l) = items.next() {
      l.sink.send(ev).unwrap();
      let Reply { resub, outcome } = l.source.recv().unwrap();
      if resub {
        listeners.push(l);
      }
      match outcome {
        Ok(res) => {
          listeners.extend(items);
          return Some(res);
        },
        Err(next) => {
          ev = next;
        },
      }
    }
    None
  }

  pub fn get_one<V>(&self, mut filter: impl FnMut(&T) -> bool, f: impl FnOnce(T) -> (U, V)) -> V {
    let mut listeners = self.listeners.lock().unwrap();
    let (sink, request) = sync_channel(0);
    let (response, source) = sync_channel(0);
    listeners.push(Listener { sink, source });
    mem::drop(listeners);
    loop {
      let t = request.recv().unwrap();
      if filter(&t) {
        let (u, v) = f(t);
        response.send(Reply { resub: false, outcome: Ok(u) }).unwrap();
        return v;
      }
      response.send(Reply { resub: true, outcome: Err(t) }).unwrap();
    }
  }
}
