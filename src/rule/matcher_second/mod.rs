/*
Construction:
convert pattern into hierarchy of plain, scan, middle
  - plain: accept any sequence or any non-empty sequence
  - scan: a single scalar pattern moves LTR or RTL, submatchers on either
    side
  - middle: two scalar patterns walk over all permutations of matches
    while getting progressively closer to each other

Application:
walk over the current matcher's valid options and poll the submatchers
  for each of them
*/

mod shared;
mod vec_match;
mod scal_match;
mod any_match;
mod build;

pub use shared::AnyMatcher;
pub use build::mk_matcher;