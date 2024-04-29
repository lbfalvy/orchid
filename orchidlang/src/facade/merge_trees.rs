//! Combine constants from [super::macro_runner::MacroRunner::run_macros] with
//! systems from [super::loader::Loader::systems]

use std::sync::Arc;

use hashbrown::HashMap;

use super::system::System;
use crate::error::Reporter;
use crate::foreign::inert::Inert;
use crate::foreign::to_clause::ToClause;
use crate::intermediate::ast_to_ir::ast_to_ir;
use crate::intermediate::ir_to_nort::ir_to_nort;
use crate::interpreter::nort;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::name::{NameLike, Sym};
use crate::pipeline::project::ConstReport;
use crate::sym;
use crate::tree::{ModMemberRef, TreeTransforms};
use crate::utils::unwrap_or::unwrap_or;

/// Equivalent of [crate::pipeline::project::ConstReport] for the interpreter's
/// representation, [crate::interpreter::nort].
#[derive(Clone)]
pub struct NortConst {
  /// Comments associated with the constant which may affect its interpretation
  pub comments: Vec<Arc<String>>,
  /// Location of the definition, if known
  pub location: CodeLocation,
  /// Value assigned to the constant
  pub value: nort::Expr,
}
impl NortConst {
  /// Convert into NORT constant from AST constant
  pub fn convert_from(value: ConstReport, reporter: &Reporter) -> NortConst {
    let module = Sym::new(value.name.split_last().1[..].iter())
      .expect("Constant names from source are at least 2 long");
    let location = CodeLocation::new_src(value.range.clone(), value.name);
    let nort = match ast_to_ir(value.value, value.range, module.clone()) {
      Ok(ir) => ir_to_nort(&ir),
      Err(e) => {
        reporter.report(e);
        Inert(0).to_expr(location.clone())
      },
    };
    Self { value: nort, location, comments: value.comments }
  }
}

/// Combine a list of symbols loaded from source and the constant trees from
/// each system.
pub fn merge_trees<'a: 'b, 'b>(
  source: impl IntoIterator<Item = (Sym, ConstReport)>,
  systems: impl IntoIterator<Item = &'b System<'a>> + 'b,
  reporter: &Reporter,
) -> HashMap<Sym, NortConst> {
  let mut out = HashMap::new();
  for (name, rep) in source.into_iter() {
    out.insert(name.clone(), NortConst::convert_from(rep, reporter));
  }
  for system in systems {
    let const_module = system.constants.unwrap_mod_ref();
    const_module.search_all((), |stack, node, ()| {
      let c = unwrap_or!(node => ModMemberRef::Item; return);
      let location = CodeLocation::new_gen(CodeGenInfo::details(
        sym!(facade::merge_tree),
        format!("system.name={}", system.name),
      ));
      let value = c.clone().gen_nort(stack.clone(), location.clone());
      let crep = NortConst { value, comments: vec![], location };
      out.insert(Sym::new(stack.unreverse()).expect("root item is forbidden"), crep);
    });
  }
  out
}
