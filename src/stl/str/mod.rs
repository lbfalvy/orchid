mod char_at;
mod concatenate;

use crate::interner::Interner;
use crate::pipeline::ConstTree;

pub fn str(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("concatenate"),
    ConstTree::xfn(concatenate::Concatenate),
  )])
}
