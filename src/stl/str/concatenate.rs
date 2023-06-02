use super::super::litconv::with_str;
use crate::define_fn;
use crate::representations::interpreted::Clause;
use crate::representations::{Literal, Primitive};

define_fn! {expr=x in
  /// Append a string to another
  pub Concatenate {
    a: String as with_str(x, |s| Ok(s.clone())),
    b: String as with_str(x, |s| Ok(s.clone()))
  } => {
    Ok(Clause::P(Primitive::Literal(Literal::Str(a.to_owned() + &b))))
  }
}
