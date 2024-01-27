//! `std::exit_status` Exit status of a program or effectful subprogram.
//!
//! There is no support for custom codes, and the success/failure state can be
//! inspected.

use std::process::ExitCode;

use crate::foreign::fn_bridge::constructors::xfn_1ary;
use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tree::{atom_leaf, ConstTree};

/// An Orchid equivalent to Rust's binary exit status model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitStatus {
  /// unix exit code 0
  Success,
  /// unix exit code 1
  Failure,
}
impl ExitStatus {
  /// Convert to Rust-land [ExitCode]
  pub fn code(self) -> ExitCode {
    match self {
      Self::Success => ExitCode::SUCCESS,
      Self::Failure => ExitCode::FAILURE,
    }
  }
}

impl InertPayload for ExitStatus {
  const TYPE_STR: &'static str = "ExitStatus";
}

pub(super) fn exit_status_lib() -> ConstTree {
  let is_success = |es: Inert<ExitStatus>| Inert(es.0 == ExitStatus::Success);
  ConstTree::ns("std::exit_status", [ConstTree::tree([
    ("success", atom_leaf(Inert(ExitStatus::Success))),
    ("failure", atom_leaf(Inert(ExitStatus::Failure))),
    ("is_success", atom_leaf(xfn_1ary(is_success))),
  ])])
}
