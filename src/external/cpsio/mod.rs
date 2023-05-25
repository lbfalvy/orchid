use crate::interner::Interner;
use crate::pipeline::ConstTree;

mod debug;
mod io;
mod panic;
mod print;
mod readline;

pub use io::{handle, IO};

pub fn cpsio(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("print"), ConstTree::xfn(print::Print2)),
    (i.i("readline"), ConstTree::xfn(readline::Readln2)),
    (i.i("debug"), ConstTree::xfn(debug::Debug2)),
    (i.i("panic"), ConstTree::xfn(panic::Panic1)),
  ])
}
