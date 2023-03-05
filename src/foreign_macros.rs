
#[allow(unused)] // for the doc comments
use crate::representations::Primitive;
#[allow(unused)] // for the doc comments
use crate::foreign::{Atomic, ExternFn};
#[allow(unused)] // for the doc comments
use std::any::Any;
#[allow(unused)] // for the doc comments
use std::hash::Hash;
#[allow(unused)] // for the doc comments
use dyn_clone::DynClone;
#[allow(unused)] // for the doc comments
use std::fmt::Debug;

/// A macro that generates the straightforward, syntactically invariant part of implementing
/// [Atomic]. Implemented fns are [Atomic::as_any], [Atomic::definitely_eq] and [Atomic::hash].
/// 
/// It depends on [Eq] and [Hash]
#[macro_export]
macro_rules! atomic_defaults {
  () => {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn definitely_eq(&self, _other: &dyn std::any::Any) -> bool {
      _other.downcast_ref().map(|o| self == o).unwrap_or(false)
    }
    fn hash(&self, mut hasher: &mut dyn std::hash::Hasher) { <Self as Hash>::hash(self, &mut hasher) }
  };
}

/// A macro that generates implementations of [Atomic] to simplify the development of
/// external bindings for Orchid.
/// 
/// The macro depends on implementations of [AsRef<Clause>] and [From<(&Self, Clause)>] for
/// extracting the clause to be processed and then reconstructing the [Atomic]. Naturally,
/// supertraits of [Atomic] are also dependencies. These are [Any], [Debug] and [DynClone].
/// 
/// The simplest form just requires the typename to be specified. This additionally depends on an
/// implementation of [ExternFn] because after the clause is fully normalized it returns `Self`
/// wrapped in a [Primitive::ExternFn]. It is intended for intermediary
/// stages of the function where validation and the next state are defined in [ExternFn::apply].
/// 
/// ```
/// atomic_impl!(Multiply1)
/// ```
/// 
/// The last stage of the function should use the extended form of the macro which takes an
/// additional closure to explicitly describe what happens when the argument is fully processed.
/// 
/// ```
/// // excerpt from the exact implementation of Multiply
/// atomic_impl!(Multiply0, |Self(a, cls): &Self| {
///   let b: Numeric = cls.clone().try_into().map_err(AssertionError::into_extern)?;
///   Ok(*a * b).into())
/// })
/// ```
/// 
#[macro_export]
macro_rules! atomic_impl {
  ($typ:ident) => {
    atomic_impl!{$typ, |this: &Self| Ok(Clause::P(Primitive::ExternFn(Box::new(this.clone()))))}
  };
  ($typ:ident, $next_phase:expr) => {
    impl Atomic for $typ {
      $crate::atomic_defaults!{}
      fn run_once(&self) -> Result<Clause, $crate::representations::interpreted::InternalError> {
        match <Self as AsRef<Clause>>::as_ref(self).run_once() {
          Err(InternalError::NonReducible) => {
            ($next_phase)(self)
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
              .map_err(InternalError::Runtime)
          }
          Ok(arg) => Ok(Clause::P(Primitive::Atom(Atom::new(
            <Self as From<(&Self, Clause)>>::from((self, arg))
          )))),
          Err(e) => Err(e),
        }
      }
      fn run_n_times(&self, n: usize) -> Result<(Clause, usize), $crate::representations::interpreted::RuntimeError> {
        match <Self as AsRef<Clause>>::as_ref(self).run_n_times(n) {
          Ok((arg, k)) if k == n => Ok((Clause::P(Primitive::Atom(Atom::new(
            <Self as From<(&Self, Clause)>>::from((self, arg))))), k)),
          Ok((arg, k)) => {
            let intermediate = <Self as From<(&Self, Clause)>>::from((self, arg));
            ($next_phase)(&intermediate)
              .map(|cls| (cls, k))
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
          }
          Err(e) => Err(e),
        }
      }
      fn run_to_completion(&self) -> Result<Clause, $crate::representations::interpreted::RuntimeError> {
        match <Self as AsRef<Clause>>::as_ref(self).run_to_completion() {
          Ok(arg) => {
            let intermediate = <Self as From<(&Self, Clause)>>::from((self, arg));
            ($next_phase)(&intermediate)
              .map_err($crate::representations::interpreted::RuntimeError::Extern)
          },
          Err(e) => Err(e)
        }
      }
    }
  };
}

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

/// Implement [ExternFn] with a closure that produces an [Atomic] from a reference to self
/// and a closure. This can be used in conjunction with [atomic_impl] and [atomic_redirect]
/// to normalize the argument automatically before using it.
#[macro_export]
macro_rules! externfn_impl {
  ($typ:ident, $next_atomic:expr) => {
    impl ExternFn for $typ {
      fn name(&self) -> &str {stringify!($typ)}
      fn apply(&self, c: Clause) -> Result<Clause, Rc<dyn ExternError>> {
        match ($next_atomic)(self, c) { // ? casts the result but we want to strictly forward it
          Ok(r) => Ok(Clause::P(Primitive::Atom(Atom::new(r)))),
          Err(e) => Err(e)
        }
      }
    }
  };
}