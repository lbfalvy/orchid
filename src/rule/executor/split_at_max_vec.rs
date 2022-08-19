use mappable_rc::Mrc;
use itertools::Itertools;

use crate::expression::{Expr, Clause};
use crate::utils::{mrc_derive, mrc_try_derive};

pub type MaxVecSplit = (Mrc<[Expr]>, (Mrc<str>, usize, bool), Mrc<[Expr]>);
/// Derive the details of the central vectorial and the two sides from a slice of Expr's
pub fn split_at_max_vec(pattern: Mrc<[Expr]>) -> Option<MaxVecSplit> {
    let rngidx = pattern.iter().position_max_by_key(|ex| {
        if let Expr(Clause::Placeh{vec: Some((prio, _)), ..}, _) = ex {
            *prio as i64
        } else { -1 }
    })?;
    let left = mrc_derive(&pattern, |p| &p[0..rngidx]);
    let placeh = mrc_derive(&pattern, |p| &p[rngidx].0);
    let right = if rngidx == pattern.len() {
        mrc_derive(&pattern, |x| &x[0..1])
    } else {
        mrc_derive(&pattern, |x| &x[rngidx + 1..])
    };
    mrc_try_derive(&placeh, |p| {
        if let Clause::Placeh{key, vec: Some(_)} = p {
            Some(key)
        } else {None} // Repeated below on unchanged data
    }).map(|key| {
        let key = mrc_derive(&key, String::as_str);
        if let Clause::Placeh{vec: Some((prio, nonzero)), ..} = placeh.as_ref() {
            (left, (key, *prio, *nonzero), right)
        }
        else {panic!("Impossible branch")} // Duplicate of above
    })
}
