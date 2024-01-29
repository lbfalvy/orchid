use std::ffi::OsString;

use crate::foreign::atom::Atomic;
use crate::foreign::error::ExternResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::to_clause::ToClause;
use crate::foreign::try_from_expr::TryFromExpr;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::interpreter::nort::{Clause, Expr};
use crate::libs::std::string::OrcString;
use crate::location::CodeLocation;

impl InertPayload for OsString {
  const TYPE_STR: &'static str = "OsString";
}
impl TryFromExpr for OsString {
  fn from_expr(exi: Expr) -> ExternResult<Self> { Ok(Inert::from_expr(exi)?.0) }
}
impl ToClause for OsString {
  fn to_clause(self, _: CodeLocation) -> Clause { Inert(self).atom_cls() }
}

pub fn os_to_string(os: Inert<OsString>) -> Result<Inert<OrcString>, Inert<OsString>> {
  os.0.into_string().map(|s| Inert(s.into())).map_err(Inert)
}

pub fn string_to_os(str: Inert<OrcString>) -> Inert<OsString> { Inert(str.0.get_string().into()) }

pub fn os_print(os: Inert<OsString>) -> Inert<OrcString> {
  Inert(os.0.to_string_lossy().to_string().into())
}

pub fn os_string_lib() -> ConstTree {
  ConstTree::ns("system::fs", [ConstTree::tree([
    xfn_ent("os_to_string", [os_to_string]),
    xfn_ent("string_to_os", [string_to_os]),
    xfn_ent("os_print", [os_print]),
  ])])
}
