use itertools::Itertools;

use super::shared::{AnyMatcher, ScalMatcher, VecMatcher};
use crate::ast::{Clause, PHClass, Placeholder};
use crate::interner::Tok;
use crate::rule::matcher::RuleExpr;
use crate::rule::vec_attrs::vec_attrs;
use crate::utils::Side;

pub type MaxVecSplit<'a> =
  (&'a [RuleExpr], (Tok<String>, u64, bool), &'a [RuleExpr]);

/// Derive the details of the central vectorial and the two sides from a
/// slice of Expr's
fn split_at_max_vec(pattern: &[RuleExpr]) -> Option<MaxVecSplit> {
  let rngidx = pattern.iter().position_max_by_key(|expr| {
    vec_attrs(expr).map(|attrs| attrs.1 as i64).unwrap_or(-1)
  })?;
  let (left, not_left) = pattern.split_at(rngidx);
  let (placeh, right) = not_left
    .split_first()
    .expect("The index of the greatest element must be less than the length");
  vec_attrs(placeh).map(|attrs| (left, attrs, right))
}

fn scal_cnt<'a>(iter: impl Iterator<Item = &'a RuleExpr>) -> usize {
  iter.take_while(|expr| vec_attrs(expr).is_none()).count()
}

pub fn mk_any(pattern: &[RuleExpr]) -> AnyMatcher {
  let left_split = scal_cnt(pattern.iter());
  if pattern.len() <= left_split {
    return AnyMatcher::Scalar(mk_scalv(pattern));
  }
  let (left, not_left) = pattern.split_at(left_split);
  let right_split = not_left.len() - scal_cnt(pattern.iter().rev());
  let (mid, right) = not_left.split_at(right_split);
  AnyMatcher::Vec {
    left: mk_scalv(left),
    mid: mk_vec(mid),
    right: mk_scalv(right),
  }
}

/// Pattern MUST NOT contain vectorial placeholders
fn mk_scalv(pattern: &[RuleExpr]) -> Vec<ScalMatcher> {
  pattern.iter().map(mk_scalar).collect()
}

/// Pattern MUST start and end with a vectorial placeholder
fn mk_vec(pattern: &[RuleExpr]) -> VecMatcher {
  debug_assert!(!pattern.is_empty(), "pattern cannot be empty");
  debug_assert!(
    pattern.first().map(vec_attrs).is_some(),
    "pattern must start with a vectorial"
  );
  debug_assert!(
    pattern.last().map(vec_attrs).is_some(),
    "pattern must end with a vectorial"
  );
  let (left, (key, _, nonzero), right) = split_at_max_vec(pattern)
    .expect("pattern must have vectorial placeholders at least at either end");
  let r_sep_size = scal_cnt(right.iter());
  let (r_sep, r_side) = right.split_at(r_sep_size);
  let l_sep_size = scal_cnt(left.iter().rev());
  let (l_side, l_sep) = left.split_at(left.len() - l_sep_size);
  let main = VecMatcher::Placeh { key, nonzero };
  match (left, right) {
    (&[], &[]) => VecMatcher::Placeh { key, nonzero },
    (&[], _) => VecMatcher::Scan {
      direction: Side::Left,
      left: Box::new(main),
      sep: mk_scalv(r_sep),
      right: Box::new(mk_vec(r_side)),
    },
    (_, &[]) => VecMatcher::Scan {
      direction: Side::Right,
      left: Box::new(mk_vec(l_side)),
      sep: mk_scalv(l_sep),
      right: Box::new(main),
    },
    (..) => {
      let mut key_order = l_side
        .iter()
        .chain(r_side.iter())
        .filter_map(vec_attrs)
        .collect::<Vec<_>>();
      key_order.sort_by_key(|(_, prio, _)| -(*prio as i64));
      VecMatcher::Middle {
        left: Box::new(mk_vec(l_side)),
        left_sep: mk_scalv(l_sep),
        mid: Box::new(main),
        right_sep: mk_scalv(r_sep),
        right: Box::new(mk_vec(r_side)),
        key_order: key_order.into_iter().map(|(n, ..)| n).collect(),
      }
    },
  }
}

/// Pattern MUST NOT be a vectorial placeholder
fn mk_scalar(pattern: &RuleExpr) -> ScalMatcher {
  match &pattern.value {
    Clause::P(p) => ScalMatcher::P(p.clone()),
    Clause::Name(n) => ScalMatcher::Name(*n),
    Clause::Placeh(Placeholder { name, class }) => {
      debug_assert!(
        !matches!(class, PHClass::Vec { .. }),
        "Scalar matcher cannot be built from vector pattern"
      );
      ScalMatcher::Placeh(*name)
    },
    Clause::S(c, body) => ScalMatcher::S(*c, Box::new(mk_any(body))),
    Clause::Lambda(arg, body) =>
      ScalMatcher::Lambda(Box::new(mk_any(arg)), Box::new(mk_any(body))),
  }
}

#[cfg(test)]
mod test {
  use std::rc::Rc;

  use super::mk_any;
  use crate::ast::{Clause, PHClass, Placeholder};
  use crate::interner::{InternedDisplay, Interner};

  #[test]
  fn test_scan() {
    let i = Interner::new();
    let pattern = vec![
      Clause::Placeh(Placeholder {
        class: PHClass::Vec { nonzero: false, prio: 0 },
        name: i.i("::prefix"),
      })
      .into_expr(),
      Clause::Name(i.i(&[i.i("prelude"), i.i("do")][..])).into_expr(),
      Clause::S(
        '(',
        Rc::new(vec![
          Clause::Placeh(Placeholder {
            class: PHClass::Vec { nonzero: false, prio: 0 },
            name: i.i("expr"),
          })
          .into_expr(),
          Clause::Name(i.i(&[i.i("prelude"), i.i(";")][..])).into_expr(),
          Clause::Placeh(Placeholder {
            class: PHClass::Vec { nonzero: false, prio: 1 },
            name: i.i("rest"),
          })
          .into_expr(),
        ]),
      )
      .into_expr(),
      Clause::Placeh(Placeholder {
        class: PHClass::Vec { nonzero: false, prio: 0 },
        name: i.i("::suffix"),
      })
      .into_expr(),
    ];
    let matcher = mk_any(&pattern);
    println!("{}", matcher.bundle(&i));
  }
}
