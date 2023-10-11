use unicode_segmentation::UnicodeSegmentation;

use crate::error::RuntimeError;
use crate::foreign::{
  xfn_1ary, xfn_2ary, xfn_3ary, Atomic, ToClause, XfnResult,
};
use crate::interner::Interner;
use crate::interpreted::Clause;
use crate::representations::OrcString;
use crate::systems::codegen::{opt, tuple};
use crate::utils::iter_find;
use crate::ConstTree;

pub fn len(s: OrcString) -> XfnResult<usize> { Ok(s.graphemes(true).count()) }

pub fn size(s: OrcString) -> XfnResult<usize> { Ok(s.as_bytes().len()) }

/// Append a string to another
pub fn concatenate(a: OrcString, b: OrcString) -> XfnResult<String> {
  Ok(a.get_string() + b.as_str())
}

pub fn slice(s: OrcString, i: usize, len: usize) -> XfnResult<String> {
  let graphs = s.as_str().graphemes(true);
  if i == 0 {
    return Ok(graphs.take(len).collect::<String>());
  }
  let mut prefix = graphs.skip(i - 1);
  if prefix.next().is_none() {
    return Err(RuntimeError::ext(
      "Character index out of bounds".to_string(),
      "indexing string",
    ));
  }
  let mut count = 0;
  let ret = (prefix.take(len))
    .map(|x| {
      count += 1;
      x
    })
    .collect::<String>();
  if count == len {
    Ok(ret)
  } else {
    RuntimeError::fail(
      "Character index out of bounds".to_string(),
      "indexing string",
    )
  }
}

pub fn find(haystack: OrcString, needle: OrcString) -> XfnResult<Clause> {
  let haystack_graphs = haystack.as_str().graphemes(true);
  let found = iter_find(haystack_graphs, needle.as_str().graphemes(true));
  Ok(opt(found.map(|x| x.atom_exi())))
}

pub fn split(s: OrcString, i: usize) -> XfnResult<Clause> {
  let mut graphs = s.as_str().graphemes(true);
  let a = graphs.by_ref().take(i).collect::<String>();
  let b = graphs.collect::<String>();
  Ok(tuple([a.to_exi(), b.to_exi()]))
}

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("string"),
    ConstTree::tree([
      (i.i("concat"), ConstTree::xfn(xfn_2ary(concatenate))),
      (i.i("slice"), ConstTree::xfn(xfn_3ary(slice))),
      (i.i("find"), ConstTree::xfn(xfn_2ary(find))),
      (i.i("split"), ConstTree::xfn(xfn_2ary(split))),
      (i.i("len"), ConstTree::xfn(xfn_1ary(len))),
      (i.i("size"), ConstTree::xfn(xfn_1ary(size))),
    ]),
  )])
}
