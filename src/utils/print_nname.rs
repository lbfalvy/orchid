use itertools::Itertools;

use crate::interner::Interner;
use crate::Sym;

/// Print symbols to :: delimited strings
pub fn sym2string(t: Sym, i: &Interner) -> String {
  i.r(t).iter().map(|t| i.r(*t)).join("::")
}
