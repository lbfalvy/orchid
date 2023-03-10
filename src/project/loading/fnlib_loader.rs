use itertools::Itertools;
use ordered_float::NotNan;

use crate::parse::FileEntry;
use crate::representations::Primitive;
use crate::utils::{one_mrc_slice, mrc_empty_slice};
use crate::foreign::ExternFn;
use crate::ast::{Rule, Expr, Clause};

use super::{Loader, ext_loader};

pub fn fnlib_loader(src: Vec<(&'static str, Box<dyn ExternFn>)>) -> impl Loader {
  let entries = src.into_iter().map(|(name, xfn)| FileEntry::Rule(Rule {
      source: one_mrc_slice(Expr(Clause::Name{
        local: Some(name.to_string()),
        qualified: one_mrc_slice(name.to_string())
      }, mrc_empty_slice())),
      prio: NotNan::try_from(0.0f64).unwrap(),
      target: one_mrc_slice(Expr(Clause::P(Primitive::ExternFn(xfn)), mrc_empty_slice()))
    }, true))
    .collect_vec();
  ext_loader(entries)
}