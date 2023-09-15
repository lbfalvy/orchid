//! A variant of [std::any::Provider]

use std::any::Any;

/// A request for a value of an unknown type
pub struct Request<'a>(&'a mut dyn Any);
impl<'a> Request<'a> {
  pub fn can_serve<T: 'static>(&self) -> bool { self.0.is::<Option<T>>() }

  pub fn serve<T: 'static>(&mut self, value: T) { self.serve_with(|| value) }

  pub fn serve_with<T: 'static>(&mut self, provider: impl FnOnce() -> T) {
    if let Some(slot) = self.0.downcast_mut() {
      *slot = provider();
    }
  }
}

/// Trait for objects that can respond to type-erased commands. This trait is
/// a dependency of `Atomic` but the implementation can be left empty.
pub trait Responder {
  fn respond(&self, _request: Request) {}
}

pub fn request<T: 'static>(responder: &(impl Responder + ?Sized)) -> Option<T> {
  let mut slot = None;
  responder.respond(Request(&mut slot));
  slot
}
