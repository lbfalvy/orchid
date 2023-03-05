
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

/// Define a new struct and implement [ExternFn] for it so that it combines with an argument into
/// the struct identified by `$next` with a single field called `clause`.
/// Also generates doc comment for the new struct
#[macro_export]
macro_rules! xfn_initial {
  (#[$prefix:meta] $name:ident, $next:ident) => {
    paste::paste!{
      #[$prefix]
      #[doc = "\n\nNext state: [" [<$next:camel>] "]"]
      #[derive(Clone)]
      pub struct [<$name:camel>];
      externfn_impl!([<$name:camel>], |_:&Self, clause: Clause| Ok([<$next:camel>]{ clause }));
    }
  };
}

/// Define a struct with a field `clause: Clause` that forwards atomic reductions to that field and
/// then converts itself to an [ExternFn] which combines with an argument into the struct identified
/// by `$next` with all fields copied over, `$nname` converted via `$ntransform` and the
/// argument assigned to a field called `clause`.
#[macro_export]
macro_rules! xfn_middle {
  (#[$prefix:meta] $prev:ident, $name:ident, $next:ident, (
    $nname:ident : $ntype:ty
    $(, $fname:ident : $ftype:ty)*
  ), $transform:expr) => {
    paste::paste!{
      #[$prefix]
      #[doc = "\n\nPrev state: [" [<$prev:camel>] "], Next state: [" [<$next:camel>] "]"]
      #[derive(Debug, Clone, PartialEq, Hash)]
      pub struct [<$name:camel>] {
        clause: Clause,
        $($fname: $ftype),*
      }
      atomic_redirect!([<$name:camel>], clause);
      atomic_impl!([<$name:camel>]);
      externfn_impl!([<$name:camel>], |this: &Self, clause: Clause| {
        let $nname: $ntype = match ($transform)(&this.clause) {
          Ok(a) => a,
          Err(e) => return Err(e)
        };
        Ok([<$next:camel>]{
          clause,
          $nname,
          $($fname: this.$fname.clone()),*
        })
      });
    }
  };
}

#[macro_export]
macro_rules! xfn_last {
  (#[$prefix:meta] $prev:ident, $name:ident, (
    $nname:ident : $ntype:ty
    $(, $fname:ident : $ftype:ty)*
  ), $transform:expr, $operation:expr) => {
    paste!{
      #[$prefix]
      #[doc = "\n\nPrev state: [" [<$prev:camel>] "]" ]
      #[derive(Debug, Clone, PartialEq, Hash)]
      pub struct [<$name:camel>] {
        clause: Clause,
        $($fname: $ftype),*
      }
    }
    atomic_redirect!([<$name:camel>], clause);
    atomic_impl!([<$name:camel>], |this: &Self| {
      let $nname: $ntype = match ($ntransform)(&this.clause) {
        Ok(a) => a,
        Err(e) => return Err(e)
      };
      $(
        let $fname = &this.$fname;
      )*
      $operation
    });
  };
}

#[macro_export]
macro_rules! reverse_proplist {
  (
    #[$nprefix:meta] $nname:ident : $ntype:ty : $ntransform:expr
    $(, #[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr)*
  ) => {
    reverse_proplist!($($fname : $ftype : $ftransform),*)
    $nname : $ntype : $ntranform
  };
}

#[macro_export]
macro_rules! xfn_make_head {
  (
    #[$cprefix:meta] $nname:ident : $ntype:ty : $ntransform:expr
    , #[$nprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
    $(, #[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr)+
  ) => {
    xfn_make_head!(
      #[$nprefix] $cname : $ctype : $ctransform
      $(, #[$fprefix] $fname : $ftype : $ftransform)+
    )
  }; // skip through all intermediate rows
  (
    #[$cprefix:meta] $nname:ident : $ntype:ty : $ntransform:expr
    , #[$nprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
  ) => {
    paste!{ 
      xfn_initial!(#[$cprefix] $cname, $nname)
    }
  } // process first two rows
}

#[macro_export]
macro_rules! xfn_make_middle {
  (
    #[$nprefix:meta] $nname:ident : $ntype:ty : $ntransform:expr
    , #[$cprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
    , #[$pprefix:meta] $pname:ident : $ptype:ty : $ptransform:expr
    $(, #[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr)*
  ) => {
    // repeat on tail
    xfn_make_middle!(
      #[$cprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
      , #[$pprefix:meta] $pname:ident : $ptype:ty : $ptransform:expr
      $(, #[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr)*
    )
    xfn_middle!(#[$cprefix] $pname, $cname, $nname (
      $cname : $ctype
      , $pname : $ptype
      $(, $fname : $ftype )*
    ), $ctransform) // note that the "next" row is not included
  };
  (
    #[$nprefix:meta] $nname:ident : $ntype:ty : $ntransform:expr
    , #[$cprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
  ) => {}; // discard last two rows (xfn_make_head handles those)
}

#[macro_export]
macro_rules! xfn_make_last {
  ((
    #[$cprefix:meta] $cname:ident : $ctype:ty : $ctransform:expr
    , #[$pprefix:meta] $pname:ident : $ptype:ty : $ptransform:expr
    $(, #[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr)*
  ), $operation:expr) => {
    xfn_last!(
      #[$cprefix] $pname, $cname, (
        $cname : $ctype
        , $pname : $ptype
        $(, $fname : $ftype)*
      ), $ctransform, $operation
    )
  };
}

#[macro_export]
macro_rules! xfn_reversed {
  (
    ( $(#[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr),* ),
    $operation:expr
  ) => {
    xfn_make_head!($(#[$fprefix] $fname : $ftype : $ftransform),*)
    xfn_make_middle!(
      $(#[$fprefix] $fname : $ftype : $ftransform),*
    )
    xfn_make_last(
      ( $(#[$fprefix] $fname : $ftype : $ftransform),* ),
      $operation
    )
  };
}

#[macro_export]
macro_rules! xfn {
  (
    ( $(#[$fprefix:meta] $fname:ident : $ftype:ty : $ftransform:expr),* ),
    $operation:expr
  ) => {
    $crate::xfn_reversed!(
      reverse_proplist!($(#[$fprefix] $fname : $ftype : $ftransform),*),
      $operation
    );
  };
}