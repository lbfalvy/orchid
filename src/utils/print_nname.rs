use itertools::Itertools;

use crate::interner::{Interner, Token};

#[allow(unused)]
pub fn print_nname(t: Token<Vec<Token<String>>>, i: &Interner) -> String {
  i.r(t).iter().map(|t| i.r(*t)).join("::")
}

#[allow(unused)]
pub fn print_nname_seq<'a>(
  tv: impl Iterator<Item = &'a Token<Vec<Token<String>>>>,
  i: &Interner
) -> String {
  tv.map(|t| print_nname(*t, i)).join(", ")
}