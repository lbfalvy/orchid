//! `std::exit_status` Exit status of a program or effectful subprogram.
//!
//! There is no support for custom codes, and the success/failure state can be
//! inspected.

use std::process::ExitCode;

use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};

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
    atom_ent("success", [Inert(ExitStatus::Success)]),
    atom_ent("failure", [Inert(ExitStatus::Failure)]),
    xfn_ent("is_success", [is_success]),
  ])])
}
