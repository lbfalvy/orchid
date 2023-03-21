use std::fmt::Debug;

// TODO: extract to crate

/// A FILO stack that lives on the regular call stack as a linked list.
/// Mainly useful to detect loops in recursive algorithms where
/// the recursion isn't deep enough to warrant a heap-allocated set.
#[derive(Clone, Copy)]
pub struct Stackframe<'a, T> {
  pub item: T,
  pub prev: Option<&'a Stackframe<'a, T>>,
  pub len: usize
}

impl<'a, T: 'a> Stackframe<'a, T> {
  pub fn new(item: T) -> Self {
    Self {
      item,
      prev: None,
      len: 1
    }
  }
  /// Get the item owned by this listlike, very fast O(1)
  pub fn item(&self) -> &T { &self.item }
  /// Get the next link in the list, very fast O(1)
  pub fn prev(&self) -> Option<&'a Stackframe<T>> { self.prev }
  /// Construct an iterator over the listlike, very fast O(1)
  pub fn iter(&self) -> StackframeIterator<T> {
    StackframeIterator { curr: Some(self) }
  }
  pub fn push(&self, item: T) -> Stackframe<'_, T>  {
    Stackframe {
      item,
      prev: Some(self),
      len: self.len + 1
    }
  }
  #[allow(unused)]
  pub fn opush(prev: Option<&'a Self>, item: T) -> Self {
    Self {
      item,
      prev,
      len: prev.map_or(1, |s| s.len)
    }
  }
  #[allow(unused)]
  pub fn len(&self) -> usize { self.len }
  #[allow(unused)]
  pub fn pop(&self, count: usize) -> Option<&Self> {
    if count == 0 {Some(self)}
    else {self.prev.expect("Index out of range").pop(count - 1)}
  }
  #[allow(unused)]
  pub fn opop(cur: Option<&Self>, count: usize) -> Option<&Self> {
    if count == 0 {cur}
    else {Self::opop(cur.expect("Index out of range").prev, count - 1)}
  }
  #[allow(unused)]
  pub fn o_into_iter(curr: Option<&Self>) -> StackframeIterator<T> {
    StackframeIterator { curr }
  }
}

impl<'a, T> Debug for Stackframe<'a, T> where T: Debug {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Substack")?;
    f.debug_list().entries(self.iter()).finish()
  }
}

pub struct StackframeIterator<'a, T> {
  curr: Option<&'a Stackframe<'a, T>>
}

impl<'a, T> StackframeIterator<'a, T> {
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
}

impl<'a, T> Iterator for StackframeIterator<'a, T> {
  type Item = &'a T;
  fn next(&mut self) -> Option<&'a T> {
    let curr = self.curr?;
    let item = curr.item();
    let prev = curr.prev();
    self.curr = prev;
    Some(item)
  }
}