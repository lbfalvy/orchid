use std::fmt::Debug;

/// Implement a FILO stack that lives on the regular call stack as a linked list.
/// Mainly useful to detect loops in recursive algorithms where the recursion isn't
/// deep enough to warrant a heap-allocated set
#[derive(Clone, Copy)]
pub struct Stackframe<'a, T> {
    pub item: T,
    pub prev: Option<&'a Stackframe<'a, T>>
}

impl<'a, T: 'a> Stackframe<'a, T> {
    pub fn new(item: T) -> Self {
        Self {
            item,
            prev: None
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
            prev: Some(self)
        }
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
