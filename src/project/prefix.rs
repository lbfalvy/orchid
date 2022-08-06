use crate::expression::{Expr, Clause};

/// Replaces the first element of a name with the matching prefix from a prefix map

/// Produce a Token object for any value of Expr other than Typed.
/// Called by [#prefix] which handles Typed.
fn prefix_clause(
    expr: &Clause,
    namespace: &Vec<String>
) -> Clause {
    match expr {
        Clause::S(c, v) => Clause::S(*c, v.iter().map(|e| prefix_expr(e, namespace)).collect()),
        Clause::Auto(name, typ, body) => Clause::Auto(
            name.clone(),
            typ.iter().map(|e| prefix_expr(e, namespace)).collect(),
            body.iter().map(|e| prefix_expr(e, namespace)).collect(),
        ),
        Clause::Lambda(name, typ, body) => Clause::Lambda(
            name.clone(),
            typ.iter().map(|e| prefix_expr(e, namespace)).collect(),
            body.iter().map(|e| prefix_expr(e, namespace)).collect(),
        ),
        Clause::Name{local, qualified} => Clause::Name{
            local: local.clone(),
            qualified: namespace.iter().chain(qualified.iter()).cloned().collect()
        },
        x => x.clone()
    }
}

/// Produce an Expr object for any value of Expr
pub fn prefix_expr(Expr(clause, typ): &Expr, namespace: &Vec<String>) -> Expr {
    Expr(
        prefix_clause(clause, namespace),
        typ.as_ref().map(|e| Box::new(prefix_expr(e, namespace)))
    )
}
