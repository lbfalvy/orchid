//! `std::tuple` A vector-based sequence for storing short sequences.

use std::fmt::Debug;
use std::sync::Arc;

use once_cell::sync::Lazy;

use super::conv::TO_STRING;
use super::protocol::Tag;
use super::reflect::refer;
use crate::foreign::error::{AssertionError, ExternResult};
use crate::foreign::fn_bridge::constructors::{xfn_1ary, xfn_2ary};
use crate::foreign::fn_bridge::Thunk;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::try_from_expr::WithLoc;
use crate::gen::tpl;
use crate::gen::tree::{atom_leaf, leaf, ConstTree};
use crate::interpreter::nort::Expr;
use crate::location::{CodeGenInfo, CodeLocation};
use crate::utils::ddispatch::Request;
use crate::utils::pure_seq::pushed;

static TUPLE_TAG: Lazy<Tag> = Lazy::new(|| {
  let location =
    CodeLocation::Gen(CodeGenInfo::no_details("stdlib::tuple::tag"));
  Tag::new("tuple", [(
    TO_STRING.id(),
    refer("std::tuple::to_string_impl").to_expr(location),
  )])
});

/// A short contiquous random access sequence of Orchid values.
#[derive(Clone)]
pub struct Tuple(pub Arc<Vec<Expr>>);
impl InertPayload for Tuple {
  const TYPE_STR: &'static str = "tuple";
  fn respond(&self, mut request: Request) {
    request.serve_with(|| TUPLE_TAG.clone())
  }
}
impl Debug for Tuple {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Tuple")?;
    f.debug_list().entries(self.0.iter()).finish()
  }
}

fn length(tuple: Inert<Tuple>) -> Inert<usize> { Inert(tuple.0.0.len()) }

fn pick(
  WithLoc(loc, tuple): WithLoc<Inert<Tuple>>,
  idx: Inert<usize>,
) -> ExternResult<Expr> {
  (tuple.0.0.get(idx.0).cloned()).ok_or_else(|| {
    let msg = format!("{} <= {idx}", tuple.0.0.len());
    AssertionError::ext(loc, "Tuple index out of bounds", msg)
  })
}

fn push(Inert(tuple): Inert<Tuple>, item: Thunk) -> Inert<Tuple> {
  let items = Arc::try_unwrap(tuple.0).unwrap_or_else(|a| (*a).clone());
  Inert(Tuple(Arc::new(pushed(items, item.0))))
}

pub(super) fn tuple_lib() -> ConstTree {
  ConstTree::ns("std", [ConstTree::tree([TUPLE_TAG.as_tree_ent([
    ("empty", leaf(tpl::V(Inert(Tuple(Arc::new(Vec::new())))))),
    ("length", atom_leaf(xfn_1ary(length))),
    ("pick", atom_leaf(xfn_2ary(pick))),
    ("push", atom_leaf(xfn_2ary(push))),
  ])])])
}
