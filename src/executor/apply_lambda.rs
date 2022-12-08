use mappable_rc::Mrc;

use crate::utils::collect_to_mrc;

use super::super::representations::typed::{Clause, Expr};

pub fn apply_lambda(id: u64, value: Mrc<Expr>, body: Mrc<Expr>) -> Mrc<Expr> {
    apply_lambda_expr_rec(id, value, Mrc::clone(&body))
        .unwrap_or(body)
}

fn apply_lambda_expr_rec(
    id: u64, value: Mrc<Expr>, expr: Mrc<Expr>
) -> Option<Mrc<Expr>> {
    let Expr(clause, typ) = expr.as_ref();
    match clause {
        Clause::Argument(arg_id) if *arg_id == id => {
            let full_typ = collect_to_mrc(
                value.1.iter()
                .chain(typ.iter())
                .cloned()
            );
            Some(Mrc::new(Expr(value.0.to_owned(), full_typ)))
        }
        cl => {
            apply_lambda_clause_rec(id, value, clause.clone())
                .map(|c| Mrc::new(Expr(c, Mrc::clone(typ))))
        }
    }
}

fn apply_lambda_clause_rec(
    id: u64, value: Mrc<Expr>, clause: Clause
) -> Option<Clause> {
    match clause {
        // Only element actually manipulated
        Clause::Argument(id) => panic!(
            "apply_lambda_expr_rec is supposed to eliminate this case"),
        // Traverse, yield Some if either had changed.
        Clause::Apply(f, x) => {
            let new_f = apply_lambda_expr_rec(
                id, Mrc::clone(&value), Mrc::clone(&f)
            );
            let new_x = apply_lambda_expr_rec(
                id, value, Mrc::clone(&x)
            );
            match (new_f, new_x) { // Mind the shadows
                (None, None) => None,
                (None, Some(x)) => Some(Clause::Apply(f, x)),
                (Some(f), None) => Some(Clause::Apply(f, x)),
                (Some(f), Some(x)) => Some(Clause::Apply(f, x))
            }
        },
        Clause::Lambda(own_id, t, b) => apply_lambda__traverse_param(id, value, own_id, t, b, Clause::Lambda),
        Clause::Auto(own_id, t, b) => apply_lambda__traverse_param(id, value, own_id, t, b, Clause::Auto),
        // Leaf nodes
        Clause::Atom(_) | Clause::ExternFn(_) | Clause::Literal(_) => None
    }
}

fn apply_lambda__traverse_param(
    id: u64, value: Mrc<Expr>,
    own_id: u64, t: Option<Mrc<Clause>>, b: Mrc<Expr>,
    wrap: impl Fn(u64, Option<Mrc<Clause>>, Mrc<Expr>) -> Clause
) -> Option<Clause> {
    let new_t = t.and_then(|t| apply_lambda_clause_rec(
        id, Mrc::clone(&value), t.as_ref().clone()
    ));
    // Respect shadowing
    let new_b = if own_id == id {None} else {
        apply_lambda_expr_rec(id, value, Mrc::clone(&b))
    };
    match (new_t, new_b) { // Mind the shadows
        (None, None) => None,
        (None, Some(b)) => Some(wrap(own_id, t, b)),
        (Some(t), None) => Some(wrap(own_id, Some(Mrc::new(t)), b)),
        (Some(t), Some(b)) => Some(wrap(own_id, Some(Mrc::new(t)), b))
    }
}