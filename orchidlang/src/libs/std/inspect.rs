use crate::foreign::fn_bridge::Thunk;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::nort::Expr;

pub fn inspect_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([
    xfn_ent("inspect", [|x: Thunk| {
      eprintln!("{}", x.0);
      x.0
    }]),
    xfn_ent("tee", [|x: Expr| {
      eprintln!("{x}");
      x
    }]),
  ])])
}
