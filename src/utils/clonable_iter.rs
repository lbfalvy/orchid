use std::sync::{Arc, Mutex};

use crate::utils::take_with_output::take_with_output;

enum State<I: Iterator> {
  End,
  Head(I),
  Cont(Clonable<I>, I::Item),
}

/// Wraps a regular iterator and buffers previously emitted elements, to ensure
/// that clones of this iterator emit the same sequence of elements
/// independently of each other. Note that this ruins pretty much all of Rust's
/// iterator-related optimizations and allocates each buffered element on the
/// heap.
pub struct Clonable<I: Iterator>(Arc<Mutex<State<I>>>);
impl<I> Clonable<I>
where
  I: Iterator,
  I::Item: Clone,
{
  pub fn new(iter: impl IntoIterator<IntoIter = I, Item = I::Item>) -> Self {
    Self::wrap(State::Head(iter.into_iter()))
  }

  fn wrap(s: State<I>) -> Self { Self(Arc::new(Mutex::new(s))) }
}

impl<I> Iterator for Clonable<I>
where
  I: Iterator,
  I::Item: Clone,
{
  type Item = I::Item;
  fn next(&mut self) -> Option<Self::Item> {
    take_with_output(self, |Self(arc)| match Arc::try_unwrap(arc) {
      Ok(mutex) => match mutex.into_inner().unwrap() {
        State::End => (Self::wrap(State::End), None),
        State::Cont(next, data) => (next, Some(data)),
        State::Head(mut iter) => match iter.next() {
          None => (Self::wrap(State::End), None),
          Some(data) => (Self::wrap(State::Head(iter)), Some(data)),
        },
      },
      Err(arc) => take_with_output(&mut *arc.lock().unwrap(), |s| match s {
        State::End => (State::End, (Self::wrap(State::End), None)),
        State::Cont(next, data) =>
          (State::Cont(next.clone(), data.clone()), (next, Some(data))),
        State::Head(mut iter) => match iter.next() {
          None => (State::End, (Self::wrap(State::End), None)),
          Some(data) => {
            let head = Self::wrap(State::Head(iter));
            (State::Cont(head.clone(), data.clone()), (head, Some(data)))
          },
        },
      }),
    })
  }
  fn size_hint(&self) -> (usize, Option<usize>) {
    let mut steps = 0;
    let mut cur = self.0.clone();
    loop {
      let guard = cur.lock().unwrap();
      match &*guard {
        State::End => break (steps, Some(steps)),
        State::Head(i) => {
          let (min, max) = i.size_hint();
          break (min + steps, max.map(|s| s + steps));
        },
        State::Cont(next, _) => {
          let tmp = next.0.clone();
          drop(guard);
          cur = tmp;
          steps += 1;
        },
      }
    }
  }
}

impl<I: Iterator> Clone for Clonable<I> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}
