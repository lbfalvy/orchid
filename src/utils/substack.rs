use std::collections::VecDeque;
use std::fmt::Debug;

// TODO: extract to crate

/// A FILO stack that lives on the regular call stack as a linked list.
/// Mainly useful to detect loops in recursive algorithms where
/// the recursion isn't deep enough to warrant a heap-allocated set.
#[derive(Clone, Copy)]
pub struct Stackframe<'a, T> {
  pub item: T,
  pub prev: &'a Substack<'a, T>,
  pub len: usize
}

#[derive(Clone, Copy)]
pub enum Substack<'a, T> {
  Frame(Stackframe<'a, T>),
  Bottom
}

impl<'a, T> Substack<'a, T> {
  /// Convert the substack into an option of stackframe
  pub fn opt(&'a self) -> Option<&'a Stackframe<'a, T>> { match self {
    Self::Frame(f) => Some(f),
    Self::Bottom => None
  } }
  /// Construct an iterator over the listlike, very fast O(1)
  pub fn iter(&self) -> SubstackIterator<T> {
    SubstackIterator { curr: self }
  }
  pub fn push(&'a self, item: T) -> Self {
    Self::Frame(self.new_frame(item))
  }
  pub fn new_frame(&'a self, item: T) -> Stackframe<'a, T> {
    Stackframe {
      item,
      prev: &self,
      len: self.opt().map_or(1, |s| s.len)
    }
  }
  pub fn pop(&'a self, count: usize) -> Option<&'a Stackframe<'a, T>> {
    if let Self::Frame(p) = self {
      if count == 0 {Some(&p)}
      else {p.prev.pop(count - 1)}
    } else {None}
  }
  pub fn len(&self) -> usize { match self {
    Self::Frame(f) => f.len,
    Self::Bottom => 0
   } }
}

impl<'a, T> Debug for Substack<'a, T> where T: Debug {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Substack")?;
    f.debug_list().entries(self.iter()).finish()
  }
}

pub struct SubstackIterator<'a, T> {
  curr: &'a Substack<'a, T>
}

impl<'a, T> SubstackIterator<'a, T> {
  #[allow(unused)]
  pub fn first_some<U, F>(&mut self, f: F) -> Option<U>
  where F: Fn(&T) -> Option<U> {
    while let Some(x) = self.next() {
      if let Some(result) = f(x) {
        return Some(result)
      }
    }
    None
  }

  /// Returns an iterator that starts from the bottom of the stack
  /// and ends at the current position. This moves all items to the
  /// heap by copying them to a [Vec]
  pub fn rev_vec_clone(self) -> Vec<T> where T: Clone {
    let mut deque = VecDeque::with_capacity(self.curr.len());
    for item in self { deque.push_front(item.clone()) }
    deque.into()
  }
}

impl<'a, T> Copy for SubstackIterator<'a, T> {}
impl<'a, T> Clone for SubstackIterator<'a, T> {
  fn clone(&self) -> Self {
    Self { curr: self.curr }
  }
}

impl<'a, T> Iterator for SubstackIterator<'a, T> {
  type Item = &'a T;
  fn next(&mut self) -> Option<&'a T> {
    let curr = self.curr.opt()?;
    let item = &curr.item;
    let prev = curr.prev;
    self.curr = prev;
    Some(item)
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    (self.curr.len(), Some(self.curr.len()))
  }
}



