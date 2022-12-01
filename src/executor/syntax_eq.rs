use std::collections::HashMap;
use std::hash::{Hasher, Hash};

use mappable_rc::Mrc;

use crate::utils::ProtoMap;

use super::super::representations::typed::{Clause, Expr};
use super::super::utils::Stackframe;

pub fn swap<T, U>((t, u): (T, U)) -> (U, T) { (u, t) }

// All data to be forwarded during recursion about one half of a unification task
#[derive(Clone)]
struct UnifHalfTask<'a> {
    /// The expression to be unified
    expr: &'a Expr,
    /// Auto parameters with their values from the opposite side
    ctx: &'a ProtoMap<'a, usize, Mrc<Expr>>,
    /// Stores whether a given relative upreference is auto or lambda
    is_auto: Option<Stackframe<'a, bool>>,
    /// Metastack of explicit arguments not yet resolved. An explicit will always exactly pair with
    /// the first auto below it. Disjoint autos always bubble with a left-to-right precedence.
    explicits: Option<Stackframe<'a, Mrc<Expr>>>
}

impl<'a> UnifHalfTask<'a> {
    fn push_auto(&self, body: &Expr) -> (Self, bool) {
        if let Some(Stackframe{ prev, .. }) = self.explicits {(
            Self{
                expr: body,
                is_auto: Stackframe::opush(&self.is_auto, false),
                explicits: prev.cloned(),
                ..*self
            },
            true
        )} else {(
            Self{
                expr: body,
                is_auto: Stackframe::opush(&self.is_auto, true),
                ..*self
            },
            false
        )}
    }

    fn push_lambda(&self, body: &Expr) -> Self {Self{
        expr: body,
        is_auto: Stackframe::opush(&self.is_auto, false),
        ..*self
    }}

    fn push_explicit(&self, subexpr: &Expr, arg: Mrc<Expr>) -> Self {Self{
        expr: subexpr,
        explicits: Stackframe::opush(&self.explicits, arg),
        ..*self
    }}

    fn push_expr(&self, f: &Expr) -> Self {Self{
        expr: f,
        ..*self
    }}
}

#[derive(Default)]
struct UnifResult {
    /// Collected identities for the given side
    context: HashMap<usize, Mrc<Expr>>,
    /// Number of explicits to be eliminated from task before forwarding to the next branch
    usedExplicits: usize,
}

impl UnifResult {
    fn useExplicit(self) -> Self{Self{
        usedExplicits: self.usedExplicits + 1,
        context: self.context.clone()
    }}

    fn dropUsedExplicits(&mut self, task: &mut  UnifHalfTask) {
        task.explicits = task.explicits.map(|s| {
            s.pop(self.usedExplicits).expect("More explicits used than provided")
        }).cloned();
        self.usedExplicits = 0;
    }
}

/// Ascertain syntactic equality. Syntactic equality means that
/// - lambda elements are verbatim equal
/// - auto constraints are pairwise syntactically equal after sorting
/// 
/// Context associates variables with subtrees resolved on the opposite side
pub fn unify_syntax_rec( // the stacks store true for autos, false for lambdas
    ltask@UnifHalfTask{ expr: lexpr@Expr(lclause, _), .. }: UnifHalfTask,
    rtask@UnifHalfTask{ expr: rexpr@Expr(rclause, _), .. }: UnifHalfTask
) -> Option<(UnifResult, UnifResult)> {
    // Ensure that ex1 is a value-level construct
    match lclause {
        Clause::Auto(_, body) => {
            let res = unify_syntax_rec(ltask.push_auto(body).0, rtask);
            return if ltask.explicits.is_some() {
                res.map(|(r1, r2)| (r1.useExplicit(), r2))
            } else {res}
        }
        Clause::Explicit(subexpr, arg) => {
            let new_ltask = ltask.push_explicit(subexpr, Mrc::clone(arg));
            return unify_syntax_rec(new_ltask, rtask)
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