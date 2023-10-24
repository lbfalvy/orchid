use std::cmp::Ordering;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use crate::foreign::{xfn_2ary, InertAtomic};
use crate::{ConstTree, Interner, Sym};

#[derive(Debug, Clone)]
pub struct SymbolName(pub Sym);
impl InertAtomic for SymbolName {
  fn type_str() -> &'static str { "SymbolName" }
}

// #[derive(Debug, Clone)]
// pub struct GetSymName;
// impl ExternFn for GetSymName {
//   fn name(&self) -> &str { "GetSymName" }
//   fn apply(
//     self: Box<Self>,
//     arg: ExprInst,
//     _: Context,
//   ) -> XfnResult<Clause> { arg.inspect(|c| match c { Clause::Constant(name)
//     => Ok(SymbolName(name.clone()).atom_cls()), _ =>
//     AssertionError::fail(arg.location(), "is not a constant name"), })
//   }
// }

#[derive(Clone)]
pub struct RefEqual(Arc<u8>);
impl RefEqual {
  pub fn new() -> Self { Self(Arc::new(0u8)) }
  pub fn id(&self) -> usize { &*self.0 as *const u8 as usize }
}
impl Debug for RefEqual {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("RefEqual").field(&self.id()).finish()
  }
}
impl InertAtomic for RefEqual {
  fn type_str() -> &'static str { "RefEqual" }
  fn strict_eq(&self, other: &Self) -> bool { self == other }
}
impl Eq for RefEqual {}
impl PartialEq for RefEqual {
  fn eq(&self, other: &Self) -> bool { self.id() == other.id() }
}
impl Ord for RefEqual {
  fn cmp(&self, other: &Self) -> Ordering { self.id().cmp(&other.id()) }
}
impl PartialOrd for RefEqual {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}
impl Hash for RefEqual {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.id().hash(state) }
}

pub fn reflect(i: &Interner) -> ConstTree {
  // ConstTree::tree([])
  ConstTree::namespace(
    [i.i("reflect")],
    ConstTree::tree([(
      i.i("ref_equal"),
      ConstTree::xfn(xfn_2ary(|l: RefEqual, r: RefEqual| Ok(l.id() == r.id()))),
    )]),
  )
}
