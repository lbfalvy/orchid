//! A simplified, stable variant of `std::any::Provider`.

use std::any::Any;

/// A request for a value of an unknown type
pub struct Request<'a>(&'a mut dyn Any);
impl<'a> Request<'a> {
  /// Checks if a value of the given type would serve the request, and the
  /// request had not yet been served
  pub fn can_serve<T: 'static>(&self) -> bool {
    self.0.downcast_ref::<Option<T>>().map_or(false, Option::is_none)
  }

  /// Serve a value if it's the correct type
  pub fn serve<T: 'static>(&mut self, value: T) { self.serve_with::<T>(|| value) }

  /// Invoke the callback to serve the request only if the return type matches
  pub fn serve_with<T: 'static>(&mut self, provider: impl FnOnce() -> T) {
    if let Some(slot) = self.0.downcast_mut::<Option<T>>() {
      if slot.is_none() {
        *slot = Some(provider());
      }
    }
  }
}

/// Trait for objects that can respond to type-erased commands. This trait is
/// a dependency of `Atomic` but the implementation can be left empty.
pub trait Responder {
  /// Try to provide as many types as we support
  fn respond(&self, _request: Request) {}
}

/// Request a specific contract type from a responder
pub fn request<T: 'static>(responder: &(impl Responder + ?Sized)) -> Option<T> {
  let mut slot = None;
  responder.respond(Request(&mut slot));
  slot
}
