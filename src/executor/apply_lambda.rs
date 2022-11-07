use mappable_rc::Mrc;

use super::super::representations::typed::{Clause, Expr};

pub fn apply_lambda(body: Mrc<Expr>, arg: Mrc<Expr>) -> Mrc<Expr> {
    apply_lambda_expr_rec(Mrc::clone(&body), arg, 0)
        .unwrap_or(body)
}

fn apply_lambda_expr_rec(
    item: Mrc<Expr>, arg: Mrc<Expr>, depth: usize
) -> Option<Mrc<Expr>> {
    let Expr(clause, typ) = item.as_ref();
    apply_lambda_clause_rec(clause.clone(), arg, depth)
        .map(|c| Mrc::new(Expr(c, Mrc::clone(typ))))
}

fn apply_lambda_clause_rec(
    clause: Clause, arg: Mrc<Expr>, depth: usize
) -> Option<Clause> {
    match clause {
        // Only element actually manipulated
        Clause::Argument(d) => {
            if d == depth {Some(arg.0.clone())} // Resolve reference
            // Application eliminates a layer of indirection
            else if d > depth {Some(Clause::Argument(d - 1))}
            else {None} // Undisturbed ancestry
        }
        // Traverse, yield Some if either had changed.
        Clause::Apply(f, x) => apply_lambda__traverse_call(arg, depth, f, x, Clause::Apply),
        Clause::Explicit(f, t) => apply_lambda__traverse_call(arg, depth, f, t, Clause::Explicit),
        Clause::Lambda(t, b) => apply_lambda__traverse_param(arg, depth, t, b, Clause::Lambda),
        Clause::Auto(t, b) => apply_lambda__traverse_param(arg, depth, t, b, Clause::Auto),
        // Leaf nodes
        Clause::Atom(_) | Clause::ExternFn(_) | Clause::Literal(_) => None
    }
}

fn apply_lambda__traverse_call(
    arg: Mrc<Expr>, depth: usize, f: Mrc<Expr>, x: Mrc<Expr>,
    wrap: impl Fn(Mrc<Expr>, Mrc<Expr>) -> Clause
) -> Option<Clause> {
    let new_f = apply_lambda_expr_rec(Mrc::clone(&f), Mrc::clone(&arg), depth);
    let new_x = apply_lambda_expr_rec(Mrc::clone(&x), arg, depth);
    match (new_f, new_x) {
        (None, None) => None,
        (None, Some(x)) => Some(wrap(f, x)),
        (Some(f), None) => Some(wrap(f, x)),
        (Some(f), Some(x)) => Some(wrap(f, x))
    }
}

fn apply_lambda__traverse_param(
    arg: Mrc<Expr>, depth: usize, t: Option<Mrc<Clause>>, b: Mrc<Expr>,
    wrap: impl Fn(Option<Mrc<Clause>>, Mrc<Expr>) -> Clause
) -> Option<Clause> {
    let new_t = t.as_ref().and_then(|t| {
        apply_lambda_clause_rec(t.as_ref().clone(), Mrc::clone(&arg), depth)
    });
    let new_b = apply_lambda_expr_rec(Mrc::clone(&b), arg, depth + 1);
    match (new_t, new_b) {
        (None, None) => None,
        (None, Some(b)) => Some(Clause::Lambda(t, b)),
        (Some(t), None) => Some(Clause::Lambda(Some(Mrc::new(t)), b)),
        (Some(t), Some(b)) => Some(Clause::Lambda(Some(Mrc::new(t)), b))
    }
}