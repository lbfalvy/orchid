use crate::foreign::{xfn_1ary, InertAtomic};
use crate::{ConstTree, Interner};

/// An Orchid equivalent to Rust's binary exit status model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitStatus {
  /// unix exit code 0
  Success,
  /// unix exit code 1
  Failure,
}

impl InertAtomic for ExitStatus {
  fn type_str() -> &'static str { "ExitStatus" }
}

pub fn exit_status(i: &Interner) -> ConstTree {
  let is_success = |es: ExitStatus| Ok(es == ExitStatus::Success);
  ConstTree::namespace(
    [i.i("exit_status")],
    ConstTree::tree([
      (i.i("success"), ConstTree::atom(ExitStatus::Success)),
      (i.i("failure"), ConstTree::atom(ExitStatus::Failure)),
      (i.i("is_success"), ConstTree::xfn(xfn_1ary(is_success))),
    ]),
  )
}
