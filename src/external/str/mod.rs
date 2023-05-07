mod concatenate;
mod char_at;

use crate::{pipeline::ConstTree, interner::Interner};

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("concatenate"), ConstTree::xfn(concatenate::Concatenate2))
  ])
}