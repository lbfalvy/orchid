use crate::{interner::Interner, pipeline::ConstTree};

mod print;
mod readline;
mod debug;
mod panic;

pub fn cpsio(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("print"), ConstTree::xfn(print::Print2)),
    (i.i("readline"), ConstTree::xfn(readline::Readln2)),
    (i.i("debug"), ConstTree::xfn(debug::Debug2)),
    (i.i("panic"), ConstTree::xfn(panic::Panic1))
  ])
}