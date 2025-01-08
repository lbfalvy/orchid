use std::cmp::Ordering;

use itertools::Itertools;

use super::scal_match::scalv_match;
use super::shared::VecMatcher;
use orchid_base::name::Sym;
use crate::{macros::MacTree, rule::state::{MatchState, StateEntry}};

#[must_use]
pub fn vec_match<'a>(
  matcher: &VecMatcher,
  seq: &'a [MacTree],
  save_loc: &impl Fn(Sym) -> bool,
) -> Option<MatchState<'a>> {
  match matcher {
    VecMatcher::Placeh { key, nonzero } => {
      if *nonzero && seq.is_empty() {
        return None;
      }
      Some(MatchState::from_ph(key.clone(), StateEntry::Vec(seq)))
    },
    VecMatcher::Scan { left, sep, right, direction } => {
      if seq.len() < sep.len() {
        return None;
      }
      for lpos in direction.walk(0..=seq.len() - sep.len()) {
        let rpos = lpos + sep.len();
        let state = vec_match(left, &seq[..lpos], save_loc)
          .and_then(|s| Some(s.combine(scalv_match(sep, &seq[lpos..rpos], save_loc)?)))
          .and_then(|s| Some(s.combine(vec_match(right, &seq[rpos..], save_loc)?)));
        if let Some(s) = state {
          return Some(s);
        }
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
        .filter_map(|(i, window)| scalv_match(left_sep, window, save_loc).map(|s| (i, s)))
        .collect::<Vec<_>>();
      // Valid locations for the right separator
      let rposv = seq[left_sep.len()..]
        .windows(right_sep.len())
        .enumerate()
        .filter_map(|(i, window)| scalv_match(right_sep, window, save_loc).map(|s| (i, s)))
        .collect::<Vec<_>>();
      // Valid combinations of locations for the separators
      let mut pos_pairs = lposv
        .into_iter()
        .cartesian_product(rposv)
        .filter(|((lpos, _), (rpos, _))| lpos + left_sep.len() <= *rpos)
        .map(|((lpos, lstate), (rpos, rstate))| (lpos, rpos, lstate.combine(rstate)))
        .collect::<Vec<_>>();
      // In descending order of size
      pos_pairs.sort_by_key(|(l, r, _)| -((r - l) as i64));
      let eql_clusters = pos_pairs.into_iter().chunk_by(|(al, ar, _)| ar - al);
      for (_gap_size, cluster) in eql_clusters.into_iter() {
        let best_candidate = cluster
          .into_iter()
          .filter_map(|(lpos, rpos, state)| {
            Some(
              state
                .combine(vec_match(left, &seq[..lpos], save_loc)?)
                .combine(vec_match(mid, &seq[lpos + left_sep.len()..rpos], save_loc)?)
                .combine(vec_match(right, &seq[rpos + right_sep.len()..], save_loc)?),
            )
          })
          .max_by(|a, b| {
            for key in key_order {
              let alen = a.ph_len(key).expect("key_order references scalar or missing");
              let blen = b.ph_len(key).expect("key_order references scalar or missing");
              match alen.cmp(&blen) {
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
