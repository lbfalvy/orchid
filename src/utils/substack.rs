
/// Implement a FILO stack that lives on the regular call stack as a linked list.
/// Mainly useful to detect loops in recursive algorithms where the recursion isn't
/// deep enough to warrant a heap-allocated set
#[derive(Debug, Clone, Copy)]
pub struct Substack<'a, T> {
    pub item: T,
    pub prev: Option<&'a Self>
}

impl<'a, T> Substack<'a, T> {
    pub fn item(&self) -> &T { &self.item }
    pub fn prev(&self) -> Option<&'a Substack<'a, T>> { self.prev }

    pub fn new(item: T) -> Self {
        Self {
            item,
            prev: None
        }
    }
    pub fn push(&'a self, item: T) -> Self {
        Self {
            item,
            prev: Some(self)
        }
    }
    pub fn iter(&'a self) -> SubstackIterator<'a, T> {
        SubstackIterator { curr: Some(self) }
    }
}

pub struct SubstackIterator<'a, T> {
    curr: Option<&'a Substack<'a, T>>
}

impl<'a, T> Iterator for SubstackIterator<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        let Substack{ item, prev } = self.curr?;
        self.curr = *prev;
        Some(item)
    }
}