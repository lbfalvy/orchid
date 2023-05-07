use crate::{ast::Expr, rule::state::State};

use super::{shared::AnyMatcher, scal_match::scalv_match, vec_match::vec_match};

pub fn any_match<'a>(matcher: &AnyMatcher, seq: &'a [Expr])
-> Option<State<'a>>
{
  match matcher {
    AnyMatcher::Scalar(scalv) => scalv_match(scalv, seq),
    AnyMatcher::Vec{ left, mid, right } => {
      let left_split = left.len();
      let right_split = seq.len() - right.len();
      let mut state = scalv_match(left, &seq[..left_split])?;
      state.extend(scalv_match(right, &seq[right_split..])?);
      state.extend(vec_match(mid, &seq[left_split..right_split])?);
      Some(state)
    }
  }
}