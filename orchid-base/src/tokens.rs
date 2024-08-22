pub use api::Paren;

use crate::api;

pub const PARENS: &[(char, char, Paren)] =
  &[('(', ')', Paren::Round), ('[', ']', Paren::Square), ('{', '}', Paren::Curly)];
