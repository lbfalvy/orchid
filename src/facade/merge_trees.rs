use std::sync::Arc;

use hashbrown::HashMap;
use never::Never;
use substack::Substack;

use super::system::System;
use crate::error::ProjectResult;
use crate::intermediate::ast_to_ir::ast_to_ir;
use crate::intermediate::ir_to_nort::ir_to_nort;
use crate::interpreter::nort;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::{Sym, VPath};
use crate::pipeline::project::ConstReport;
use crate::tree::{ModMember, ModMemberRef, TreeTransforms};

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
    out.insert(name.clone(), NortConst {
      value: ir_to_nort(&ast_to_ir(rep.value, name)?),
      location: CodeLocation::Source(rep.range),
      comments: rep.comments,
    });
  }
  for sys in systems {
    let const_module = sys.constants.unwrap_mod_ref();
    const_module.search_all((), |path, node, ()| {
      let m = if let ModMemberRef::Mod(m) = node { m } else { return };
      for (key, ent) in &m.entries {
        if let ModMember::Item(c) = &ent.member {
          let path = VPath::new(path.unreverse()).as_prefix_of(key.clone());
          let location = CodeLocation::Gen(CodeGenInfo::details(
            "constant from",
            format!("system.name={}", sys.name),
          ));
          let value = c.gen_nort(location.clone());
          let crep = NortConst { value, comments: vec![], location };
          out.insert(path.to_sym(), crep);
        }
      }
    });
  }
  Ok(out)
}
