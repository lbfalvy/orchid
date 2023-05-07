use core::fmt::Formatter;
use std::fmt::Display;

use crate::interner::Interner;

/// A variant of [std::fmt::Display] for objects that contain interned
/// strings and therefore can only be stringified in the presence of a
/// string interner
/// 
/// The functions defined here are suffixed to distinguish them from
/// the ones in Display and ToString respectively, because Rust can't
/// identify functions based on arity
pub trait InternedDisplay {
  /// formats the value using the given formatter and string interner
  fn fmt_i(&self,
    f: &mut std::fmt::Formatter<'_>,
    i: &Interner,
  ) -> std::fmt::Result;

  /// Converts the value to a string to be displayed
  fn to_string_i(&self, i: &Interner) -> String {
    // Copied from <https://doc.rust-lang.org/src/alloc/string.rs.html#2526>
    let mut buf = String::new();
    let mut formatter = Formatter::new(&mut buf);
    // Bypass format_args!() to avoid write_str with zero-length strs
    Self::fmt_i(self, &mut formatter, i)
      .expect("a Display implementation returned an error unexpectedly");
    buf
  }

  fn bundle<'a>(&'a self, interner: &'a Interner)
  -> DisplayBundle<'a, Self>
  {
    DisplayBundle { interner, data: self }
  }
}

impl<T> InternedDisplay for T where T: Display {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, _i: &Interner) -> std::fmt::Result {
    <Self as Display>::fmt(&self, f)
  }
}

pub struct DisplayBundle<'a, T: InternedDisplay + ?Sized> {
  interner: &'a Interner,
  data: &'a T
}

impl<'a, T: InternedDisplay> Display for DisplayBundle<'a, T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.data.fmt_i(f, self.interner)
  }
}