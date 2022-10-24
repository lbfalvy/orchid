use mappable_rc::Mrc;

use crate::{ast::{Expr, Clause}, utils::{collect_to_mrc, to_mrc_slice}};

/// Replaces the first element of a name with the matching prefix from a prefix map

/// Produce a Token object for any value of Expr other than Typed.
/// Called by [#prefix] which handles Typed.
fn prefix_clause(
    expr: &Clause,
    namespace: Mrc<[String]>
) -> Clause {
    match expr {
        Clause::S(c, v) => Clause::S(*c,
            collect_to_mrc(v.iter().map(|e| prefix_expr(e, Mrc::clone(&namespace))))
        ),
        Clause::Auto(name, typ, body) => Clause::Auto(
            name.clone(),
            collect_to_mrc(typ.iter().map(|e| prefix_expr(e, Mrc::clone(&namespace)))),
            collect_to_mrc(body.iter().map(|e| prefix_expr(e, Mrc::clone(&namespace)))),
        ),
        Clause::Lambda(name, typ, body) => Clause::Lambda(
            name.clone(),
            collect_to_mrc(typ.iter().map(|e| prefix_expr(e, Mrc::clone(&namespace)))),
            collect_to_mrc(body.iter().map(|e| prefix_expr(e, Mrc::clone(&namespace)))),
        ),
        Clause::Name{local, qualified} => Clause::Name{
            local: local.clone(),
            qualified: collect_to_mrc(namespace.iter().chain(qualified.iter()).cloned())
        },
        x => x.clone()
    }
}

/// Produce an Expr object for any value of Expr
pub fn prefix_expr(Expr(clause, typ): &Expr, namespace: Mrc<[String]>) -> Expr {
    Expr(
        prefix_clause(clause, Mrc::clone(&namespace)),
        to_mrc_slice(typ.iter().map(|e| prefix_clause(e, Mrc::clone(&namespace))).collect())
    )
}
