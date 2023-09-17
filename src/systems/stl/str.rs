use unicode_segmentation::UnicodeSegmentation;

use crate::interner::Interner;
use crate::representations::OrcString;
use crate::systems::codegen::{orchid_opt, tuple};
use crate::systems::RuntimeError;
use crate::utils::iter_find;
use crate::{define_fn, ConstTree, Literal};

define_fn! {
  pub Len = |x| Ok(Literal::Uint(
    (*x.downcast::<OrcString>()?)
      .graphemes(true)
      .count() as u64
  ).into());

  pub Size = |x| Ok(Literal::Uint(
    (*x.downcast::<OrcString>()?)
      .as_bytes()
      .len() as u64
  ).into());

expr=x in
  /// Append a string to another
  pub Concatenate { a: OrcString, b: OrcString } => Ok(
    Literal::Str((a.get_string() + b.as_str()).into()).into()
  );

  pub Slice { s: OrcString, i: u64, len: u64 } => {
    let graphs = s.as_str().graphemes(true);
    if i == 0 {
      let orc_str = graphs.take(len as usize).collect::<String>().into();
      Ok(Literal::Str(orc_str).into())
    } else {
      let mut prefix = graphs.skip(i as usize - 1);
      if prefix.next().is_none() {
        RuntimeError::fail(
          "Character index out of bounds".to_string(),
          "indexing string",
        )
      } else {
        let mut count = 0;
        let ret = (prefix.take(len as usize))
            .map(|x| { count+=1; x })
            .collect::<String>().into();
        if count == len {
          Ok(Literal::Str(ret).into())
        } else {
          RuntimeError::fail(
            "Character index out of bounds".to_string(),
            "indexing string"
          )
        }
      }
    }
  };

  pub Find { haystack: OrcString, needle: OrcString } => {
    let haystack_graphs = haystack.as_str().graphemes(true);
    let found = iter_find(haystack_graphs, needle.as_str().graphemes(true));
    Ok(orchid_opt(found.map(|x| Literal::Uint(x as u64).into())))
  };

  pub Split { s: OrcString, i: u64 } => {
    let mut graphs = s.as_str().graphemes(true);
    let a = graphs.by_ref().take(i as usize).collect::<String>();
    let b = graphs.collect::<String>();
    Ok(tuple([a.into(), b.into()]))
  }
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
