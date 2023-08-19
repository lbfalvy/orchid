use std::fmt::Display;
use std::rc::Rc;

use crate::foreign::ExternError;
use crate::systems::cast_exprinst::with_str;
use crate::{define_fn, ConstTree, Interner};

/// An unrecoverable error in Orchid land. Because Orchid is lazy, this only
/// invalidates expressions that reference the one that generated it.
pub struct OrchidPanic(Rc<String>);

impl Display for OrchidPanic {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Orchid code panicked: {}", self.0)
  }
}

impl ExternError for OrchidPanic {}

define_fn! {
  /// Takes a message, returns an [ExternError] unconditionally.
  Panic = |x| with_str(x, |s| {
    let msg = Rc::new(s.get_string());
    Err(OrchidPanic(msg).into_extern())
  })
}

pub fn panic(i: &Interner) -> ConstTree {
  ConstTree::tree([(i.i("panic"), ConstTree::xfn(Panic))])
}
