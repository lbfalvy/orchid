use std::rc::Rc;

use hashbrown::HashMap;

use super::matcher::RuleExpr;
use crate::ast::{Clause, Expr, PHClass, Placeholder};
use crate::interner::Tok;
use crate::utils::unwrap_or;

#[derive(Clone, Copy, Debug)]
pub enum StateEntry<'a> {
  Vec(&'a [RuleExpr]),
  Scalar(&'a RuleExpr),
}
pub type State<'a> = HashMap<Tok<String>, StateEntry<'a>>;

#[must_use]
pub fn apply_exprv(template: &[RuleExpr], state: &State) -> Vec<RuleExpr> {
  template
    .iter()
    .map(|e| apply_expr(e, state))
    .flat_map(Vec::into_iter)
    .collect()
}

#[must_use]
pub fn apply_expr(template: &RuleExpr, state: &State) -> Vec<RuleExpr> {
  let Expr { location, value } = template;
  match value {
    Clause::Atom(_) | Clause::Name(_) | Clause::ExternFn(_) =>
      vec![template.clone()],
    Clause::S(c, body) => vec![Expr {
      location: location.clone(),
      value: Clause::S(*c, Rc::new(apply_exprv(body.as_slice(), state))),
    }],
    Clause::Placeh(Placeholder { name, class }) => {
      let value = *unwrap_or!(state.get(name);
        panic!("Placeholder does not have a value in state")
      );
      match (class, value) {
        (PHClass::Scalar, StateEntry::Scalar(item)) => vec![item.clone()],
        (PHClass::Vec { .. }, StateEntry::Vec(chunk)) => chunk.to_vec(),
        _ => panic!("Type mismatch between template and state"),
      }
    },
    Clause::Lambda(arg, body) => vec![Expr {
      location: location.clone(),
      value: Clause::Lambda(
        Rc::new(apply_exprv(arg, state)),
        Rc::new(apply_exprv(&body[..], state)),
      ),
    }],
  }
}
