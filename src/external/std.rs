use crate::pipeline::ConstTree;
use crate::interner::Interner;

use super::bool::bool;
use super::cpsio::cpsio;
use super::conv::conv;
use super::str::str;
use super::num::num;

pub fn std(i: &Interner) -> ConstTree {
    cpsio(i)
    + conv(i)
    + bool(i)
    + str(i)
    + num(i)
}