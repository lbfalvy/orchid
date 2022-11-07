use std::hash::{Hasher, Hash};

use super::super::representations::typed::{Clause, Expr};
use super::super::utils::Stackframe;

/// Hash the parts of an expression that are required to be equal for syntactic equality.
pub fn partial_hash_rec<H: Hasher>(Expr(clause, _): &Expr, state: &mut H, is_auto: Stackframe<bool>) {
    match clause {
        // Skip autos and explicits
        Clause::Auto(_, body) => partial_hash_rec(body, state, is_auto.push(true)),
        Clause::Explicit(f, _) => partial_hash_rec(f, state, is_auto),
        // Annotate everything else with a prefix
        // - Recurse into the tree of lambdas and calls - classic lambda calc
        Clause::Lambda(_, body) => {
            state.write_u8(0);
            partial_hash_rec(body, state, is_auto.push(false))
        }
        Clause::Apply(f, x) => {
            state.write_u8(1);
            partial_hash_rec(f, state, is_auto);
            partial_hash_rec(x, state, is_auto);
        }
        // - Only recognize the depth of an argument if it refers to a non-auto parameter
        Clause::Argument(depth) => {
            // If the argument references an auto, acknowledge its existence
            if *is_auto.iter().nth(*depth).unwrap_or(&false) {
                state.write_u8(2)
            } else {
                state.write_u8(3);
                state.write_usize(*depth)
            }
        }
        // - Hash leaves like normal
        Clause::Literal(lit) => { state.write_u8(4); lit.hash(state) }
        Clause::Atom(at) => { state.write_u8(5); at.hash(state) }
        Clause::ExternFn(f) => { state.write_u8(6); f.hash(state) }
    }
}