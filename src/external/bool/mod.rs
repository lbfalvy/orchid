mod equals;
mod boolean;
mod ifthenelse;
pub use boolean::Boolean;

use crate::{pipeline::ConstTree, interner::Interner};


pub fn bool(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("ifthenelse"), ConstTree::xfn(ifthenelse::IfThenElse1)),
    (i.i("equals"), ConstTree::xfn(equals::Equals2))
  ])
}