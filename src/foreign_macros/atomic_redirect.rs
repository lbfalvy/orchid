#[allow(unused)]
use crate::atomic_impl;

/// Implement the traits required by [atomic_impl] to redirect run calls
/// to a field with a particular name.
#[macro_export]
macro_rules! atomic_redirect {
  ($typ:ident) => {
    impl AsMut<$crate::interpreted::ExprInst> for $typ {
      fn as_mut(&mut self) -> &mut $crate::interpreted::ExprInst { &mut self.0 }
    }
  };
  ($typ:ident, $field:ident) => {
    impl AsMut<$crate::interpreted::ExprInst> for $typ {
      fn as_mut(&mut self) -> &mut $crate::interpreted::ExprInst {
        &mut self.$field
      }
    }
  };
}
