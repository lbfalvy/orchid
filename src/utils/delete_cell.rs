use std::cell::RefCell;
use std::rc::Rc;

pub struct DeleteCell<T>(pub Rc<RefCell<Option<T>>>);
impl<T> DeleteCell<T> {
  pub fn new(t: T) -> Self { Self(Rc::new(RefCell::new(Some(t)))) }

  pub fn take(&self) -> Option<T> { self.0.borrow_mut().take() }

  pub fn clone_out(&self) -> Option<T>
  where
    T: Clone,
  {
    self.0.borrow().clone()
  }
}
impl<T> Clone for DeleteCell<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}
