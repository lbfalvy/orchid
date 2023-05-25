#[allow(unused)]
use super::atomic_impl;

/// Implement the traits required by [atomic_impl] to redirect run_* functions
/// to a field with a particular name.
#[macro_export]
macro_rules! atomic_redirect {
  ($typ:ident) => {
    impl AsRef<$crate::foreign::RcExpr> for $typ {
      fn as_ref(&self) -> &Clause {
        &self.0
      }
    }
    impl From<(&Self, $crate::foreign::RcExpr)> for $typ {
      fn from((old, clause): (&Self, Clause)) -> Self {
        Self { 0: clause, ..old.clone() }
      }
    }
  };
  ($typ:ident, $field:ident) => {
    impl AsRef<$crate::foreign::RcExpr> for $typ {
      fn as_ref(&self) -> &$crate::foreign::RcExpr {
        &self.$field
      }
    }
    impl From<(&Self, $crate::foreign::RcExpr)> for $typ {
      #[allow(clippy::needless_update)]
      fn from((old, $field): (&Self, $crate::foreign::RcExpr)) -> Self {
        Self { $field, ..old.clone() }
      }
    }
  };
}
