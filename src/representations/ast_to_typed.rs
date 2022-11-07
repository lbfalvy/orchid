use mappable_rc::Mrc;

use crate::utils::{Stackframe, to_mrc_slice, mrc_empty_slice};

use super::{ast, typed};

#[derive(Clone)]
pub enum Error {
    /// `()` as a clause is meaningless in lambda calculus
    EmptyS,
    /// Only `(...)` may be converted to typed lambdas. `[...]` and `{...}` left in the code are
    /// signs of incomplete macro execution
    BadGroup(char),
    /// `foo:bar:baz` will be parsed as `(foo:bar):baz`, explicitly specifying `foo:(bar:baz)`
    /// is forbidden and it's also meaningless since `baz` can only ever be the kind of types
    ExplicitBottomKind,
    /// Name never bound in an enclosing scope - indicates incomplete macro substitution
    Unbound(String),
    /// Namespaced names can never occur in the code, these are signs of incomplete macro execution
    Symbol,
    /// Placeholders shouldn't even occur in the code during macro execution. Something is clearly
    /// terribly wrong
    Placeholder,
    /// It's possible to try and transform the clause `(foo:bar)` into a typed clause,
    /// however the correct value of this ast clause is a typed expression (included in the error)
    /// 
    /// [expr] handles this case, so it's only really possible to get this
    /// error if you're calling [clause] directly
    ExprToClause(typed::Expr),
    /// @ tokens only ever occur between a function and a parameter
    NonInfixAt
}

/// Try to convert an expression from AST format to typed lambda
pub fn expr(expr: &ast::Expr) -> Result<typed::Expr, Error> {
    expr_rec(expr, Stackframe::new(None))
}

/// Try and convert a single clause from AST format to typed lambda
pub fn clause(clause: &ast::Clause) -> Result<typed::Clause, Error> {
    clause_rec(clause, Stackframe::new(None))
}

/// Try and convert a sequence of expressions from AST format to typed lambda 
pub fn exprv(exprv: &[ast::Expr]) -> Result<typed::Expr, Error> {
    exprv_rec(exprv, Stackframe::new(None))
}

fn apply_rec(f: typed::Expr, x: &ast::Expr, names: Stackframe<Option<&str>>)
-> Result<typed::Clause, Error> {
    if let ast::Expr(ast::Clause::Explicit(inner), empty_slice) = x {
        assert!(empty_slice.len() == 0,
            "It is assumed that Explicit nodes can never have type annotations as the \
            wrapped expression node matches all trailing colons."
        );
        let x = expr_rec(inner.as_ref(), names)?;
        Ok(typed::Clause::Explicit(Mrc::new(f), Mrc::new(x)))
    } else {
        let x = expr_rec(x, names)?;
        Ok(typed::Clause::Apply(Mrc::new(f), Mrc::new(x)))
    }
}

/// Recursive state of [exprv]
fn exprv_rec(v: &[ast::Expr], names: Stackframe<Option<&str>>) -> Result<typed::Expr, Error> {
    if v.len() == 0 {return Err(Error::EmptyS)}
    if v.len() == 1 {return expr_rec(&v[0], names)}
    let (head, tail) = v.split_at(2);
    let f = expr_rec(&head[0], names)?;
    let x = &head[1];
    // TODO this could probably be normalized, it's a third copy. 
    tail.iter().fold(
        apply_rec(f, x, names),
        |acc, e| apply_rec(
            typed::Expr(acc?, mrc_empty_slice()),
            e,
            names
        )
    ).map(|cls| typed::Expr(cls, mrc_empty_slice()))
}

/// Recursive state of [expr]
fn expr_rec(ast::Expr(val, typ): &ast::Expr, names: Stackframe<Option<&str>>)
-> Result<typed::Expr, Error> {
    let typ: Vec<typed::Clause> = typ.iter()
        .map(|c| clause_rec(c, names))
        .collect::<Result<_, _>>()?;
    if let ast::Clause::S(paren, body) = val {
        if *paren != '(' {return Err(Error::BadGroup(*paren))}
        let typed::Expr(inner, inner_t) = exprv_rec(body.as_ref(), names)?;
        let new_t = if typ.len() == 0 { inner_t } else {
            to_mrc_slice(if inner_t.len() == 0 { typ } else {
                inner_t.iter().chain(typ.iter()).cloned().collect()
            })
        };
        Ok(typed::Expr(inner, new_t))
    } else {
        Ok(typed::Expr(clause_rec(&val, names)?, to_mrc_slice(typ)))
    }
}

/// Recursive state of [clause]
fn clause_rec(cls: &ast::Clause, names: Stackframe<Option<&str>>)
-> Result<typed::Clause, Error> {
    match cls {
        ast::Clause::ExternFn(e) => Ok(typed::Clause::ExternFn(e.clone())),
        ast::Clause::Atom(a) => Ok(typed::Clause::Atom(a.clone())),
        ast::Clause::Auto(no, t, b) => Ok(typed::Clause::Auto(
            if t.len() == 0 {None} else {
                let typed::Expr(c, t) = exprv_rec(t.as_ref(), names)?;
                if t.len() > 0 {return Err(Error::ExplicitBottomKind)}
                else {Some(Mrc::new(c))}
            },
            Mrc::new(exprv_rec(b.as_ref(), names.push(no.as_ref().map(|n| &**n)))?)
        )),
        ast::Clause::Lambda(n, t, b) => Ok(typed::Clause::Lambda(
            if t.len() == 0 {None} else {
                let typed::Expr(c, t) = exprv_rec(t.as_ref(), names)?;
                if t.len() > 0 {return Err(Error::ExplicitBottomKind)}
                else {Some(Mrc::new(c))}
            },
            Mrc::new(exprv_rec(b.as_ref(), names.push(Some(&**n)))?)
        )),
        ast::Clause::Literal(l) => Ok(typed::Clause::Literal(l.clone())),
        ast::Clause::Name { local: Some(arg), .. } => Ok(typed::Clause::Argument(
            names.iter().position(|no| no == &Some(&**arg))
                .ok_or_else(|| Error::Unbound(arg.clone()))?
        )),
        ast::Clause::S(paren, entries) => {
            if *paren != '(' {return Err(Error::BadGroup(*paren))}
            let typed::Expr(val, typ) = exprv_rec(entries.as_ref(), names)?;
            if typ.len() == 0 {Ok(val)}
            else {Err(Error::ExprToClause(typed::Expr(val, typ)))}
        },
        ast::Clause::Name { local: None, .. } => Err(Error::Symbol),
        ast::Clause::Placeh { .. } => Err(Error::Placeholder),
        ast::Clause::Explicit(..) => Err(Error::NonInfixAt)
    }
}