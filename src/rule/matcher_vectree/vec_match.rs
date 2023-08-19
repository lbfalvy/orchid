use std::cmp::Ordering;

use itertools::Itertools;

use super::scal_match::scalv_match;
use super::shared::VecMatcher;
use crate::rule::matcher::RuleExpr;
use crate::rule::state::{State, StateEntry};
use crate::utils::unwrap_or;

pub fn vec_match<'a>(
  matcher: &VecMatcher,
  seq: &'a [RuleExpr],
) -> Option<State<'a>> {
  match matcher {
    VecMatcher::Placeh { key, nonzero } => {
      if *nonzero && seq.is_empty() {
        return None;
      }
      return Some(State::from([(key.clone(), StateEntry::Vec(seq))]));
    },
    VecMatcher::Scan { left, sep, right, direction } => {
      if seq.len() < sep.len() {
        return None;
      }
      for lpos in direction.walk(0..=seq.len() - sep.len()) {
        let rpos = lpos + sep.len();
        let mut state = unwrap_or!(vec_match(left, &seq[..lpos]); continue);
        state.extend(unwrap_or!(scalv_match(sep, &seq[lpos..rpos]); continue));
        state.extend(unwrap_or!(vec_match(right, &seq[rpos..]); continue));
        return Some(state);
      }
      None
    },
    // XXX predict heap space usage and allocation count
    VecMatcher::Middle { left, left_sep, mid, right_sep, right, key_order } => {
      if seq.len() < left_sep.len() + right_sep.len() {
        return None;
      }
      // Valid locations for the left separator
      let lposv = seq[..seq.len() - right_sep.len()]
        .windows(left_sep.len())
        .enumerate()
        .filter_map(|(i, window)| scalv_match(left_sep, window).map(|s| (i, s)))
        .collect::<Vec<_>>();
      // Valid locations for the right separator
      let rposv = seq[left_sep.len()..]
        .windows(right_sep.len())
        .enumerate()
        .filter_map(|(i, window)| {
          scalv_match(right_sep, window).map(|s| (i, s))
        })
        .collect::<Vec<_>>();
      // Valid combinations of locations for the separators
      let mut pos_pairs = lposv
        .into_iter()
        .cartesian_product(rposv.into_iter())
        .filter(|((lpos, _), (rpos, _))| lpos + left_sep.len() <= *rpos)
        .map(|((lpos, mut lstate), (rpos, rstate))| {
          lstate.extend(rstate);
          (lpos, rpos, lstate)
        })
        .collect::<Vec<_>>();
      // In descending order of size
      pos_pairs.sort_by_key(|(l, r, _)| -((r - l) as i64));
      let eql_clusters = pos_pairs.into_iter().group_by(|(al, ar, _)| ar - al);
      for (_gap_size, cluster) in eql_clusters.into_iter() {
        let best_candidate = cluster
          .into_iter()
          .filter_map(|(lpos, rpos, mut state)| {
            state.extend(vec_match(left, &seq[..lpos])?);
            state.extend(vec_match(mid, &seq[lpos + left_sep.len()..rpos])?);
            state.extend(vec_match(right, &seq[rpos + right_sep.len()..])?);
            Some(state)
          })
          .max_by(|a, b| {
            for key in key_order {
              let aslc = if let Some(StateEntry::Vec(s)) = a.get(key) {
                s
              } else {
                panic!("key_order references scalar or missing")
              };
              let bslc = if let Some(StateEntry::Vec(s)) = b.get(key) {
                s
              } else {
                panic!("key_order references scalar or missing")
              };
              match aslc.len().cmp(&bslc.len()) {
                Ordering::Equal => (),
                any => return any,
              }
            }
            Ordering::Equal
          });
        if let Some(state) = best_candidate {
          return Some(state);
        }
      }
      None
    },
  }
}
