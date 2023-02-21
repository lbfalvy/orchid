use mappable_rc::Mrc;

use crate::utils::{Stackframe, to_mrc_slice, mrc_empty_slice, ProtoMap, one_mrc_slice};

use super::{ast, typed, get_name::get_name};

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
  Ok(expr_rec(expr, ProtoMap::new(), None)?.0)
}

/// Try and convert a single clause from AST format to typed lambda
pub fn clause(clause: &ast::Clause) -> Result<typed::Clause, Error> {
  Ok(clause_rec(clause, ProtoMap::new(), None)?.0)
}

/// Try and convert a sequence of expressions from AST format to typed lambda 
pub fn exprv(exprv: &[ast::Expr]) -> Result<typed::Expr, Error> {
  Ok(exprv_rec(exprv, ProtoMap::new(), None)?.0)
}

const NAMES_INLINE_COUNT:usize = 3;

/// Recursive state of [exprv]
fn exprv_rec<'a>(
  v: &'a [ast::Expr],
  names: ProtoMap<&'a str, (u64, bool), NAMES_INLINE_COUNT>,
  explicits: Option<&Stackframe<Mrc<typed::Expr>>>,
) -> Result<(typed::Expr, usize), Error> {
  let (last, rest) = v.split_last().ok_or(Error::EmptyS)?;
  if rest.len() == 0 {return expr_rec(&v[0], names, explicits)}
  if let ast::Expr(ast::Clause::Explicit(inner), empty_slice) = last {
    assert!(empty_slice.len() == 0,
      "It is assumed that Explicit nodes can never have type annotations as the \
      wrapped expression node matches all trailing colons."
    );
    let (x, _) = expr_rec(inner.as_ref(), names.clone(), None)?;
    let new_explicits = Stackframe::opush(explicits, Mrc::new(x));
    let (body, used_expls) = exprv_rec(rest, names, Some(&new_explicits))?;
    Ok((body, used_expls.saturating_sub(1)))
  } else {
    let (f, f_used_expls) = exprv_rec(rest, names.clone(), explicits)?;
    let x_explicits = Stackframe::opop(explicits, f_used_expls);
    let (x, x_used_expls) = expr_rec(last, names, x_explicits)?;
    Ok((typed::Expr(
      typed::Clause::Apply(Mrc::new(f), Mrc::new(x)),
      mrc_empty_slice()
    ), x_used_expls + f_used_expls))
  }
}

/// Recursive state of [expr]
fn expr_rec<'a>(
  ast::Expr(val, typ): &'a ast::Expr,
  names: ProtoMap<&'a str, (u64, bool), NAMES_INLINE_COUNT>,
  explicits: Option<&Stackframe<Mrc<typed::Expr>>> // known explicit values
) -> Result<(typed::Expr, usize), Error> { // (output, used_explicits)
  let typ: Vec<typed::Clause> = typ.iter()
    .map(|c| Ok(clause_rec(c, names.clone(), None)?.0))
    .collect::<Result<_, _>>()?;
  if let ast::Clause::S(paren, body) = val {
    if *paren != '(' {return Err(Error::BadGroup(*paren))}
    let (typed::Expr(inner, inner_t), used_expls) = exprv_rec(body.as_ref(), names, explicits)?;
    let new_t = if typ.len() == 0 { inner_t } else {
      to_mrc_slice(if inner_t.len() == 0 { typ } else {
        inner_t.iter().chain(typ.iter()).cloned().collect()
      })
    };
    Ok((typed::Expr(inner, new_t), used_expls))
  } else {
    let (cls, used_expls) = clause_rec(&val, names, explicits)?;
    Ok((typed::Expr(cls, to_mrc_slice(typ)), used_expls))
  }
}

/// Recursive state of [clause]
fn clause_rec<'a>(
  cls: &'a ast::Clause,
  mut names: ProtoMap<&'a str, (u64, bool), NAMES_INLINE_COUNT>,
  mut explicits: Option<&Stackframe<Mrc<typed::Expr>>>
) -> Result<(typed::Clause, usize), Error> {
  match cls { // (\t:(@T. Pair T T). t \left.\right. left) @number -- this will fail
    ast::Clause::ExternFn(e) => Ok((typed::Clause::ExternFn(e.clone()), 0)),
    ast::Clause::Atom(a) => Ok((typed::Clause::Atom(a.clone()), 0)),
    ast::Clause::Auto(no, t, b) => {
      // Allocate id
      let id = get_name();
      // Pop an explicit if available
      let (value, rest_explicits) = explicits.map(
        |Stackframe{ prev, item, .. }| {
          (Some(item), *prev)
        }
      ).unwrap_or_default();
      explicits = rest_explicits;
      // Convert the type
      let typ = if t.len() == 0 {mrc_empty_slice()} else {
        let (typed::Expr(c, t), _) = exprv_rec(t.as_ref(), names.clone(), None)?;
        if t.len() > 0 {return Err(Error::ExplicitBottomKind)}
        else {one_mrc_slice(c)}
      };
      // Traverse body with extended context
      if let Some(name) = no {names.set(&&**name, (id, true))}
      let (body, used_expls) = exprv_rec(b.as_ref(), names, explicits)?;
      // Produce a binding instead of an auto if explicit was available
      if let Some(known_value) = value {
        Ok((typed::Clause::Apply(
          typed::Clause::Lambda(id, typ, Mrc::new(body)).wrap(),
          Mrc::clone(known_value)
        ), used_expls + 1))
      } else {
        Ok((typed::Clause::Auto(id, typ, Mrc::new(body)), 0))
      }
    }
    ast::Clause::Lambda(n, t, b) => {
      // Allocate id
      let id = get_name();
      // Convert the type
      let typ = if t.len() == 0 {mrc_empty_slice()} else {
        let (typed::Expr(c, t), _) = exprv_rec(t.as_ref(), names.clone(), None)?;
        if t.len() > 0 {return Err(Error::ExplicitBottomKind)}
        else {one_mrc_slice(c)}
      };
      names.set(&&**n, (id, false));
      let (body, used_expls) = exprv_rec(b.as_ref(), names, explicits)?;
      Ok((typed::Clause::Lambda(id, typ, Mrc::new(body)), used_expls))
    }
    ast::Clause::Literal(l) => Ok((typed::Clause::Literal(l.clone()), 0)),
    ast::Clause::Name { local: Some(arg), .. } => {
      let (uid, is_auto) = names.get(&&**arg)
        .ok_or_else(|| Error::Unbound(arg.clone()))?;
      let label = if *is_auto {typed::Clause::AutoArg} else {typed::Clause::LambdaArg};
      Ok((label(*uid), 0))
    }
    ast::Clause::S(paren, entries) => {
      if *paren != '(' {return Err(Error::BadGroup(*paren))}
      let (typed::Expr(val, typ), used_expls) = exprv_rec(entries.as_ref(), names, explicits)?;
      if typ.len() == 0 {Ok((val, used_expls))}
      else {Err(Error::ExprToClause(typed::Expr(val, typ)))}
    },
    ast::Clause::Name { local: None, .. } => Err(Error::Symbol),
    ast::Clause::Placeh { .. } => Err(Error::Placeholder),
    ast::Clause::Explicit(..) => Err(Error::NonInfixAt)
  }
}