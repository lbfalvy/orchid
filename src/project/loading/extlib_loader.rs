use std::rc::Rc;

use lasso::Spur;
use ordered_float::NotNan;

use crate::representations::Primitive;
use crate::representations::sourcefile::FileEntry;
use crate::foreign::ExternFn;
use crate::ast::{Rule, Clause};

use super::{Loader, ext_loader};

pub fn extlib_loader<'a, T, F>(
  fns: Vec<(&'static str, Box<dyn ExternFn>)>,
  submods: Vec<(&'static str, T)>,
  intern: &'a F
) -> impl Loader + 'a
where
  T: Loader + 'a,
  F: Fn(&str) -> Spur + 'a
{
  let entries = (
    fns.into_iter().map(|(name, xfn)| FileEntry::Rule(Rule {
      source: Rc::new(vec![
        Clause::Name(Rc::new(vec![intern(name)])).into_expr(),
      ]),
      prio: NotNan::try_from(0.0f64).unwrap(),
      target: Rc::new(vec![
        Clause::P(Primitive::ExternFn(xfn)).into_expr(),
      ])
    }, true))
  ).collect(); 
  ext_loader(entries, submods, intern)
}