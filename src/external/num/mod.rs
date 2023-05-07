mod numeric;
pub mod operators;
pub use numeric::Numeric;

use crate::{interner::Interner, pipeline::ConstTree};

pub fn num(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("add"), ConstTree::xfn(operators::add::Add2)),
    (i.i("subtract"), ConstTree::xfn(operators::subtract::Subtract2)),
    (i.i("multiply"), ConstTree::xfn(operators::multiply::Multiply2)),
    (i.i("divide"), ConstTree::xfn(operators::divide::Divide2)),
    (i.i("remainder"), ConstTree::xfn(operators::remainder::Remainder2))
  ])
}