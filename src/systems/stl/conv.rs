use ordered_float::NotNan;

use super::Numeric;
use crate::error::AssertionError;
use crate::foreign::{xfn_1ary, Atom, XfnResult};
use crate::interner::Interner;
use crate::parse::parse_num;
use crate::{ConstTree, Location, OrcString};

fn to_numeric(a: Atom) -> XfnResult<Numeric> {
  if let Some(n) = a.request::<Numeric>() {
    return Ok(n);
  }
  if let Some(s) = a.request::<OrcString>() {
    return parse_num(s.as_str())
      .map_err(|_| AssertionError::ext(Location::Unknown, "number syntax"));
  }
  AssertionError::fail(Location::Unknown, "string or number")
}

/// parse a number. Accepts the same syntax Orchid does.
pub fn to_float(a: Atom) -> XfnResult<NotNan<f64>> {
  to_numeric(a).map(|n| n.as_float())
}

/// Parse an unsigned integer. Accepts the same formats Orchid does. If the
/// input is a number, floors it.
pub fn to_uint(a: Atom) -> XfnResult<usize> {
  to_numeric(a).map(|n| match n {
    Numeric::Float(f) => f.floor() as usize,
    Numeric::Uint(i) => i,
  })
}

/// Convert a literal to a string using Rust's conversions for floats, chars and
/// uints respectively
pub fn to_string(a: Atom) -> XfnResult<OrcString> {
  a.try_downcast::<OrcString>()
    .or_else(|e| e.try_downcast::<usize>().map(|i| i.to_string().into()))
    .or_else(|e| e.try_downcast::<NotNan<f64>>().map(|i| i.to_string().into()))
    .or_else(|e| e.try_downcast::<bool>().map(|i| i.to_string().into()))
    .map_err(|_| AssertionError::ext(Location::Unknown, "string or number"))
}

pub fn conv(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("to_float"), ConstTree::xfn(xfn_1ary(to_float))),
    (i.i("to_uint"), ConstTree::xfn(xfn_1ary(to_uint))),
    (i.i("to_string"), ConstTree::xfn(xfn_1ary(to_string))),
  ])
}
