use super::super::litconv::{with_str, with_uint};
use super::super::runtime_error::RuntimeError;
use crate::interpreted::Clause;
use crate::{write_fn_step, Literal, Primitive};

write_fn_step!(pub CharAt2 > CharAt1);
write_fn_step!(
  CharAt1 {}
  CharAt0 where s = |x| with_str(x, |s| Ok(s.clone()))
);
write_fn_step!(
  CharAt0 { s: String }
  i = |x| with_uint(x, Ok)
  => {
    if let Some(c) = s.chars().nth(i as usize) {
      Ok(Clause::P(Primitive::Literal(Literal::Char(c))))
    } else {
      RuntimeError::fail(
        "Character index out of bounds".to_string(),
        "indexing string",
      )?
    }
  }
);
