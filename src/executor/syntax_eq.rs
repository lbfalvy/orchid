use std::collections::HashMap;
use std::hash::{Hasher, Hash};
use std::iter;

use mappable_rc::Mrc;

use crate::utils::{ProtoMap, Side};

use super::super::representations::typed::{Clause, Expr};
use super::super::utils::Stackframe;

pub fn swap<T, U>((t, u): (T, U)) -> (U, T) { (u, t) }

// @ @ (0, (foo 1)) ~ @ (0, 0)

// TODO:
// - get rid of leftovers from Explicit
// - adapt to new index-based system

// =@= =&= =%= =#= =$= =?= =!= =/=
// <@> <&> <%> <#> <$> <?> <!> </>
// |@| |&| |%| |#| |$| |?| |!| |/|
// {@} {&} {%} {#} {$} {?} {!} {/}
// (@) (&) (%) (#) ($) (?) (!) (/)
// [@] [&] [%] [#] [$] [?] [!] [/]

/// The context associates a given variable (by absolute index) on a given side to
/// an expression on the opposite side rooted at the specified depth.
/// The root depths are used to translate betwee de Brujin arguments and absolute indices.
struct Context(HashMap<u64, Mrc<Expr>>);
impl Context {
    fn set(&mut self, id: u64, value: Mrc<Expr>) {
        // If already defined, then it must be an argument
        if let Some(value) = self.0.get(&id) {
            if let Clause::Argument(opposite_up) ex.0
        }
    }
}

const IS_AUTO_INLINE:usize = 5;

// All data to be forwarded during recursion about one half of a unification task
#[derive(Clone)]
struct UnifHalfTask<'a> {
    /// The expression to be unified
    expr: &'a Expr,
    /// Stores whether a given uid is auto or lambda
    is_auto: ProtoMap<'a, usize, bool, IS_AUTO_INLINE>
}

impl<'a> UnifHalfTask<'a> {
    fn push_auto(&mut self, body: &Expr, key: usize) {
        self.expr = body;
        self.is_auto.set(&key, true);
    }

    fn push_lambda(&mut self, body: &Expr, key: usize) {
        self.expr = body;
        self.is_auto.set(&key, false);
    }
}

type Ctx = HashMap<usize, Mrc<Expr>>;

/// Ascertain syntactic equality. Syntactic equality means that
/// - lambda elements are verbatim equal
/// - auto constraints are pairwise syntactically equal after sorting
/// 
/// Context associates variables with subtrees resolved on the opposite side
pub fn unify_syntax_rec( // the stacks store true for autos, false for lambdas
    ctx: &mut HashMap<(Side, usize), (usize, Mrc<Expr>)>,
    ltask@UnifHalfTask{ expr: lexpr@Expr(lclause, _), .. }: UnifHalfTask,
    rtask@UnifHalfTask{ expr: rexpr@Expr(rclause, _), .. }: UnifHalfTask
) -> Option<(UnifResult, UnifResult)> {
    // Ensure that ex1 is a value-level construct
    match lclause {
        Clause::Auto(id, _, body) => {
            let res = unify_syntax_rec(ltask.push_auto(body).0, rtask);
            return if ltask.explicits.is_some() {
                res.map(|(r1, r2)| (r1.useExplicit(), r2))
            } else {res}
        }
        _ => ()
    };
    // Reduce ex2's auto handling to ex1's. In the optimizer we trust
    if let Clause::Auto(..) | Clause::Explicit(..) = rclause {
        return unify_syntax_rec(rtask, ltask).map(swap);
    }
    // Neither ex1 nor ex2 can be Auto or Explicit
    match (lclause, rclause) {
        // recurse into both
        (Clause::Lambda(_, lbody), Clause::Lambda(_, rbody)) => unify_syntax_rec(
            ltask.push_lambda(lbody),
            rtask.push_lambda(rbody)
        ),
        (Clause::Apply(lf, lx), Clause::Apply(rf, rx)) => {
            let (lpart, rpart) = unify_syntax_rec(
                ltask.push_expr(lf), 
                rtask.push_expr(rf)
            )?;
            lpart.dropUsedExplicits(&mut ltask);
            rpart.dropUsedExplicits(&mut rtask);
            unify_syntax_rec(ltask.push_expr(lx), rtask.push_expr(rx))
        }
        (Clause::Atom(latom), Clause::Atom(ratom)) => {
            if latom != ratom { None }
            else { Some((UnifResult::default(), UnifResult::default())) }
        }
        (Clause::ExternFn(lf), Clause::ExternFn(rf)) => {
            if lf != rf { None }
            else { Some((UnifResult::default(), UnifResult::default())) }
        }
        (Clause::Literal(llit), Clause::Literal(rlit)) => {
            if llit != rlit { None }
            else { Some((UnifResult::default(), UnifResult::default())) }
        }
        // TODO Select a representative
        (Clause::Argument(depth1), Clause::Argument(depth2)) => {
            !*stack1.iter().nth(*depth1).unwrap_or(&false)
            && !*stack2.iter().nth(*depth2).unwrap_or(&false)
            && stack1.iter().count() - depth1 == stack2.iter().count() - depth2
        }
        // TODO Assign a substitute
        (Clause::Argument(placeholder), _) => {

        }
    }
}

// Tricky unifications
// @A. A A 1 ~ @B. 2 B B = fails if left-authoritative
// @A. 1 A A ~ @B. B B 2
// @A. A 1 A ~ @B. B B 2
// @ 0 X 0 ~ @ 0 0 Y