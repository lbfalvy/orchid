use super::litconv::{with_str, with_uint};
use super::RuntimeError;
use crate::interner::Interner;
use crate::{define_fn, ConstTree, Literal};

define_fn! {expr=x in
  /// Append a string to another
  pub Concatenate {
    a: String as with_str(x, |s| Ok(s.clone())),
    b: String as with_str(x, |s| Ok(s.clone()))
  } => Ok(Literal::Str(a.to_owned() + b).into())
}

define_fn! {expr=x in
  pub CharAt {
    s: String as with_str(x, |s| Ok(s.clone())),
    i: u64 as with_uint(x, Ok)
  } => {
    if let Some(c) = s.chars().nth(*i as usize) {
      Ok(Literal::Char(c).into())
    } else {
      RuntimeError::fail(
        "Character index out of bounds".to_string(),
        "indexing string",
      )?
    }
  }
}

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("str"),
    ConstTree::tree([
      (i.i("concatenate"), ConstTree::xfn(Concatenate)),
      (i.i("char_at"), ConstTree::xfn(CharAt)),
    ]),
  )])
}
