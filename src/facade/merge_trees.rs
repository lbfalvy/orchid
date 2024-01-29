use std::sync::Arc;

use hashbrown::HashMap;

use super::system::System;
use crate::error::ProjectResult;
use crate::intermediate::ast_to_ir::ast_to_ir;
use crate::intermediate::ir_to_nort::ir_to_nort;
use crate::interpreter::nort;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::Sym;
use crate::pipeline::project::ConstReport;
use crate::tree::{ModMemberRef, TreeTransforms};
use crate::utils::unwrap_or::unwrap_or;

/// Equivalent of [crate::pipeline::project::ConstReport] for the interpreter's
/// representation, [crate::interpreter::nort].
pub struct NortConst {
  /// Comments associated with the constant which may affect its interpretation
  pub comments: Vec<Arc<String>>,
  /// Location of the definition, if known
  pub location: CodeLocation,
  /// Value assigned to the constant
  pub value: nort::Expr,
}

/// Combine a list of symbols loaded from source and the constant trees from
/// each system.
pub fn merge_trees<'a: 'b, 'b>(
  source: impl IntoIterator<Item = (Sym, ConstReport)> + 'b,
  systems: impl IntoIterator<Item = &'b System<'a>> + 'b,
) -> ProjectResult<impl IntoIterator<Item = (Sym, NortConst)> + 'static> {
  let mut out = HashMap::new();
  for (name, rep) in source {
    let ir = ast_to_ir(rep.value, name.clone())?;
    // if name == Sym::literal("tree::main::main") {
    //   panic!("{ir:?}");
    // }
    out.insert(name.clone(), NortConst {
      value: ir_to_nort(&ir),
      location: CodeLocation::Source(rep.range),
      comments: rep.comments,
    });
  }
  for system in systems {
    let const_module = system.constants.unwrap_mod_ref();
    const_module.search_all((), |stack, node, ()| {
      let c = unwrap_or!(node => ModMemberRef::Item; return);
      let location = CodeLocation::Gen(CodeGenInfo::details(
        "constant from",
        format!("system.name={}", system.name),
      ));
      let value = c.clone().gen_nort(stack.clone(), location.clone());
      let crep = NortConst { value, comments: vec![], location };
      out.insert(Sym::new(stack.unreverse()).expect("root item is forbidden"), crep);
    });
  }
  Ok(out)
}
