mod numeric;
use numeric::Numeric;

use std::fmt::Debug;
use std::rc::Rc;
use std::hash::Hash;

use crate::{atomic_impl, atomic_redirect, externfn_impl, xfn_initial, xfn_middle, xfn_last, xfn};
use crate::foreign::{ExternError, ExternFn, Atom, Atomic};
use crate::representations::Primitive;
use crate::representations::interpreted::{Clause, InternalError};

// xfn_initial!(
//   /// Multiply function
//   Multiply2, Multiply1
// );
// xfn_middle!(
//   /// Partially applied multiply function
//   Multiply2, Multiply1, Multiply0, (
//     a: Numeric: |c: &Clause| c.clone().try_into()
//   )
// );
// xfn_last!(
//   /// Fully applied Multiply function.
//   Multiply1, Multiply0, (
//     b: Numeric: |c: &Clause| c.clone().try_into(),
//     a: Numeric: |c: &Clause| c.clone().try_into()
//   ), Ok((*a * b).into())
// );

xfn!((
  /// Multiply function
  a: Numeric: |c: &Clause| c.clone().try_into(),
  /// Partially applied multiply function
  b: Numeric: |c: &Clause| c.clone().try_into()
), {
  Ok((*a * b).into())
});