use unicode_segmentation::UnicodeSegmentation;

use crate::foreign::{xfn_1ary, xfn_2ary, xfn_3ary, XfnResult};
use crate::interner::Interner;
use crate::interpreted::Clause;
use crate::representations::OrcString;
use crate::systems::codegen::{opt, tuple};
use crate::systems::RuntimeError;
use crate::utils::iter_find;
use crate::{ConstTree, Literal};

pub fn len(s: OrcString) -> XfnResult<u64> {
  Ok(s.graphemes(true).count() as u64)
}

pub fn size(s: OrcString) -> XfnResult<u64> { Ok(s.as_bytes().len() as u64) }

/// Append a string to another
pub fn concatenate(a: OrcString, b: OrcString) -> XfnResult<String> {
  Ok(a.get_string() + b.as_str())
}

pub fn slice(s: OrcString, i: u64, len: u64) -> XfnResult<String> {
  let graphs = s.as_str().graphemes(true);
  if i == 0 {
    return Ok(graphs.take(len as usize).collect::<String>());
  }
  let mut prefix = graphs.skip(i as usize - 1);
  if prefix.next().is_none() {
    return Err(RuntimeError::ext(
      "Character index out of bounds".to_string(),
      "indexing string",
    ));
  }
  let mut count = 0;
  let ret = (prefix.take(len as usize))
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
  Ok(opt(found.map(|x| Literal::Uint(x as u64).into())))
}

pub fn split(s: OrcString, i: u64) -> XfnResult<Clause> {
  let mut graphs = s.as_str().graphemes(true);
  let a = graphs.by_ref().take(i as usize).collect::<String>();
  let b = graphs.collect::<String>();
  Ok(tuple([a.into(), b.into()]))
}

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("str"),
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
