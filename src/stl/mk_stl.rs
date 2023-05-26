use super::bool::bool;
use super::conv::conv;
use super::cpsio::cpsio;
use super::num::num;
use super::str::str;
use crate::interner::Interner;
use crate::pipeline::ConstTree;

pub fn mk_stl(i: &Interner) -> ConstTree {
  cpsio(i) + conv(i) + bool(i) + str(i) + num(i)
}
