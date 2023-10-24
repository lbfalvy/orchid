use super::scal_match::scalv_match;
use super::shared::AnyMatcher;
use super::vec_match::vec_match;
use crate::rule::matcher::RuleExpr;
use crate::rule::state::State;
use crate::Sym;

#[must_use]
pub fn any_match<'a>(
  matcher: &AnyMatcher,
  seq: &'a [RuleExpr],
  save_loc: &impl Fn(Sym) -> bool,
) -> Option<State<'a>> {
  match matcher {
    AnyMatcher::Scalar(scalv) => scalv_match(scalv, seq, save_loc),
    AnyMatcher::Vec { left, mid, right } => {
      if seq.len() < left.len() + right.len() {
        return None;
      };
      let left_split = left.len();
      let right_split = seq.len() - right.len();
      Some(
        scalv_match(left, &seq[..left_split], save_loc)?
          .combine(scalv_match(right, &seq[right_split..], save_loc)?)
          .combine(vec_match(mid, &seq[left_split..right_split], save_loc)?),
      )
    },
  }
}
