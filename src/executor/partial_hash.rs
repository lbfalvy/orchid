use std::hash::{Hasher, Hash};

use itertools::Itertools;

use crate::utils::ProtoMap;

use super::super::representations::typed::{Clause, Expr};
use super::super::utils::Stackframe;

const PARAMETRICS_INLINE_COUNT:usize = 5;
// type Parametrics<'a> = ProtoMap<'a, u64, bool, PARAMETRICS_INLINE_COUNT>;

/// Hash the parts of an expression that are required to be equal for syntactic equality.
pub fn partial_hash_rec<H: Hasher>(
  Expr(clause, _): &Expr, state: &mut H,
  parametrics: Option<&Stackframe<u64>>
) {
  match clause {
    // Skip autos
    Clause::Auto(id, _, body) => {
      partial_hash_rec(body, state, parametrics)
    }
    // Annotate everything else with a prefix
    // - Recurse into the tree of lambdas and calls - classic lambda calc
    Clause::Lambda(id, _, body) => {
      state.write_u8(0);
      partial_hash_rec(body, state, Some(&Stackframe::opush(parametrics, *id)))
    }
    Clause::Apply(f, x) => {
      state.write_u8(1);
      partial_hash_rec(f, state, parametrics.clone());
      partial_hash_rec(x, state, parametrics);
    }
    Clause::AutoArg(..) => state.write_u8(2),
    // - Only recognize the depth of an argument if it refers to a non-auto parameter
    Clause::LambdaArg(own_id) => {
      let pos = parametrics
        .and_then(|sf| sf.iter().position(|id| id == own_id))
        .unwrap_or(usize::MAX);
      state.write_u8(3);
      state.write_usize(pos)
    }
    // - Hash leaves like normal
    Clause::Literal(lit) => { state.write_u8(4); lit.hash(state) }
    Clause::Atom(at) => { state.write_u8(5); at.hash(state) }
    Clause::ExternFn(f) => { state.write_u8(6); f.hash(state) }
  }
}