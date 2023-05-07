use crate::{interner::Interner, pipeline::ConstTree};

mod print;
mod readline;

pub fn cpsio(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("print"), ConstTree::xfn(print::Print2)),
    (i.i("readline"), ConstTree::xfn(readline::Readln2))
  ])
}