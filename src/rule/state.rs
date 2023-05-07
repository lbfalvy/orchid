use std::rc::Rc;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::interner::Token;
use crate::ast::{Expr, Clause, Placeholder, PHClass};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StateEntry<'a> {
  Vec(&'a[Expr]),
  Scalar(&'a Expr)
}
pub type State<'a> = HashMap<Token<String>, StateEntry<'a>>;

pub fn apply_exprv(template: &[Expr], state: &State) -> Vec<Expr> {
  template.iter()
    .map(|e| apply_expr(e, state))
    .flat_map(Vec::into_iter)
    .collect()
}

pub fn apply_expr(template: &Expr, state: &State) -> Vec<Expr> {
  let Expr{ location, value } = template;
  match value {
    Clause::P(_) | Clause::Name(_) => vec![template.clone()],
    Clause::S(c, body) => vec![Expr{
      location: location.clone(),
      value: Clause::S(*c, Rc::new(apply_exprv(body.as_slice(), state)))
    }],
    Clause::Placeh(Placeholder{ name, class }) => {
      let value = if let Some(&v) = state.get(name) {v}
      else {panic!("Placeholder does not have a value in state")};
      match (class, value) {
        (PHClass::Scalar, StateEntry::Scalar(item)) => vec![item.clone()],
        (PHClass::Vec{..}, StateEntry::Vec(chunk)) => chunk.to_vec(),
        _ => panic!("Type mismatch between template and state")
      }
    }
    Clause::Lambda(arg, body) => vec![Expr{
      location: location.clone(),
      value: Clause::Lambda(
        Rc::new(apply_expr(arg.as_ref(), state).into_iter()
          .exactly_one()
          .expect("Lambda arguments can only ever be scalar")
        ),
        Rc::new(apply_exprv(&body[..], state))
      )
    }]
  }
}
