//! `std::tuple` A vector-based sequence for storing short sequences.

use std::fmt;
use std::sync::Arc;

use once_cell::sync::Lazy;

use super::protocol::Tag;
use super::reflect::refer;
use crate::foreign::error::{AssertionError, RTResult};
use crate::foreign::fn_bridge::Thunk;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};
use crate::interpreter::nort::Expr;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::sym;
use crate::utils::ddispatch::Request;
use crate::utils::pure_seq::pushed;

static TUPLE_TAG: Lazy<Tag> = Lazy::new(|| {
  let location = CodeLocation::new_gen(CodeGenInfo::no_details(sym!(std::tuple)));
  Tag::new(sym!(std::tuple), [(
    sym!(std::string::conversion),
    refer("std::tuple::to_string_impl").into_expr(location),
  )])
});

/// A short contiquous random access sequence of Orchid values.
#[derive(Clone)]
pub struct Tuple(pub Arc<Vec<Expr>>);
impl InertPayload for Tuple {
  const TYPE_STR: &'static str = "tuple";
  fn respond(&self, mut request: Request) { request.serve_with(|| TUPLE_TAG.clone()) }
}
impl fmt::Debug for Tuple {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Tuple")?;
    f.debug_list().entries(self.0.iter().map(|e| &e.clause)).finish()
  }
}

fn length(tuple: Inert<Tuple>) -> Inert<usize> { Inert(tuple.0.0.len()) }

fn pick(WithLoc(loc, tuple): WithLoc<Inert<Tuple>>, idx: Inert<usize>) -> RTResult<Expr> {
  (tuple.0.0.get(idx.0).cloned()).ok_or_else(|| {
    let msg = format!("{} <= {idx}", tuple.0.0.len());
    AssertionError::ext(loc, "Tuple index out of bounds", msg)
  })
}

fn push(Inert(tuple): Inert<Tuple>, item: Thunk) -> Inert<Tuple> {
  let items = Arc::unwrap_or_clone(tuple.0);
  Inert(Tuple(Arc::new(pushed(items, item.0))))
}

pub(super) fn tuple_lib() -> ConstTree {
  ConstTree::ns("std::tuple", [TUPLE_TAG.to_tree([
    atom_ent("empty", [Inert(Tuple(Arc::new(Vec::new())))]),
    xfn_ent("length", [length]),
    xfn_ent("pick", [pick]),
    xfn_ent("push", [push]),
  ])])
}
