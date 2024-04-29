use std::sync::{Arc, Mutex};

pub struct DeleteCell<T>(pub Arc<Mutex<Option<T>>>);
impl<T> DeleteCell<T> {
  pub fn new(t: T) -> Self { Self(Arc::new(Mutex::new(Some(t)))) }
  pub fn take(&self) -> Option<T> { self.0.lock().unwrap().take() }
}
impl<T: Clone> DeleteCell<T> {
  pub fn clone_out(&self) -> Option<T> { self.0.lock().unwrap().clone() }
}
impl<T> Clone for DeleteCell<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}
