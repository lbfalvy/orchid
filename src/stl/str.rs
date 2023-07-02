use unicode_segmentation::UnicodeSegmentation;

use super::codegen::{orchid_opt, tuple};
use super::inspect::with_str;
use super::RuntimeError;
use crate::interner::Interner;
use crate::utils::iter_find;
use crate::{define_fn, ConstTree, Literal};

define_fn! {expr=x in
  /// Append a string to another
  pub Concatenate { a: String, b: String }
  => Ok(Literal::Str(a.to_owned() + b).into())
}

define_fn! {expr=x in
  pub Slice { s: String, i: u64, len: u64 } => {
    let graphs = s.graphemes(true);
    if *i == 0 {
      Ok(Literal::Str(graphs.take(*len as usize).collect()).into())
    } else {
      let mut prefix = graphs.skip(*i as usize - 1);
      if prefix.next().is_none() {
        RuntimeError::fail(
          "Character index out of bounds".to_string(),
          "indexing string",
        )
      } else {
        let mut count = 0;
        let ret = prefix
            .take(*len as usize)
            .map(|x| { count+=1; x })
            .collect();
        if count == *len {
          Ok(Literal::Str(ret).into())
        } else {
          RuntimeError::fail(
            "Character index out of bounds".to_string(),
            "indexing string"
          )
        }
      }
    }
  }
}

define_fn! {expr=x in
  pub Find { haystack: String, needle: String } => {
    let found = iter_find(haystack.graphemes(true), needle.graphemes(true));
    Ok(orchid_opt(found.map(|x| Literal::Uint(x as u64).into())))
  }
}

define_fn! {expr=x in
  pub Split { s: String, i: u64 } => {
    let mut graphs = s.graphemes(true);
    let a = graphs.by_ref().take(*i as usize).collect::<String>();
    let b = graphs.collect::<String>();
    Ok(tuple(vec![a.into(), b.into()]))
  }
}

define_fn! {
  pub Len = |x| with_str(x, |s| {
    Ok(Literal::Uint(s.graphemes(true).count() as u64).into())
  })
}

define_fn! {
  pub Size = |x| with_str(x, |s| {
    Ok(Literal::Uint(s.as_bytes().len() as u64).into())
  })
}

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("str"),
    ConstTree::tree([
      (i.i("concat"), ConstTree::xfn(Concatenate)),
      (i.i("slice"), ConstTree::xfn(Slice)),
      (i.i("find"), ConstTree::xfn(Find)),
      (i.i("split"), ConstTree::xfn(Split)),
      (i.i("len"), ConstTree::xfn(Len)),
      (i.i("size"), ConstTree::xfn(Size)),
    ]),
  )])
}
