use crate::interner::Interner;
use crate::pipeline::ConstTree;

mod command;
mod inspect;
mod panic;
mod print;
mod readline;

pub use command::{handle, IO};

pub fn io(i: &Interner, allow_impure: bool) -> ConstTree {
  let pure = ConstTree::tree([(
    i.i("io"),
    ConstTree::tree([
      (i.i("print"), ConstTree::xfn(print::Print)),
      (i.i("readline"), ConstTree::xfn(readline::ReadLn)),
      (i.i("panic"), ConstTree::xfn(panic::Panic)),
    ]),
  )]);
  if !allow_impure {
    pure
  } else {
    pure
      + ConstTree::tree([(
        i.i("io"),
        ConstTree::tree([(i.i("debug"), ConstTree::xfn(inspect::Inspect))]),
      )])
  }
}
