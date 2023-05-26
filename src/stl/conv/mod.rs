use crate::interner::Interner;
use crate::pipeline::ConstTree;

mod parse_float;
mod parse_uint;
mod to_string;

pub fn conv(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("parse_float"), ConstTree::xfn(parse_float::ParseFloat1)),
    (i.i("parse_uint"), ConstTree::xfn(parse_uint::ParseUint1)),
    (i.i("to_string"), ConstTree::xfn(to_string::ToString1)),
  ])
}
