#[allow(unused)]
use super::atomic_impl;

/// Implement the traits required by [atomic_impl] to redirect run_* functions to a field
/// with a particular name.
#[macro_export]
macro_rules! atomic_redirect {
  ($typ:ident) => {
    impl AsRef<Clause> for $typ {
      fn as_ref(&self) -> &Clause { &self.0 }
    }
    impl From<(&Self, Clause)> for $typ {
      fn from((old, clause): (&Self, Clause)) -> Self {
        Self{ 0: clause, ..old.clone() }
      }
    }
  };
  ($typ:ident, $field:ident) => {
    impl AsRef<Clause> for $typ {
      fn as_ref(&self) -> &Clause { &self.$field }
    }
    impl From<(&Self, Clause)> for $typ {
      fn from((old, $field): (&Self, Clause)) -> Self {
        Self{ $field, ..old.clone() }
      }
    }
  };
}