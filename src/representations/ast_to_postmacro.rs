use std::{rc::Rc, fmt::Display};

use crate::utils::Stackframe;

use super::{ast, postmacro};

#[derive(Clone)]
pub enum Error {
  /// `()` as a clause is meaningless in lambda calculus
  EmptyS,
  /// Only `(...)` may be converted to typed lambdas. `[...]` and `{...}` left in the code are
  /// signs of incomplete macro execution
  BadGroup(char),
  /// `foo:bar:baz` will be parsed as `(foo:bar):baz`. Explicitly specifying `foo:(bar:baz)`
  /// is forbidden and it's also meaningless since `baz` can only ever be the kind of types
  ExplicitKindOfType,
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
  ExprToClause(postmacro::Expr),
  /// @ tokens only ever occur between a function and a parameter
  NonInfixAt
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::EmptyS => write!(f, "`()` as a clause is meaningless in lambda calculus"),
      Error::BadGroup(c) => write!(f, "Only `(...)` may be converted to typed lambdas. `[...]` and `{{...}}` left in the code are signs of incomplete macro execution"),
      Error::ExplicitKindOfType => write!(f, "`foo:bar:baz` will be parsed as `(foo:bar):baz`. Explicitly specifying `foo:(bar:baz)` is forbidden and meaningless since `baz` can only ever be the kind of types"),
      Error::Unbound(name) => write!(f, "Name \"{name}\" never bound in an enclosing scope. This indicates incomplete macro substitution"),
      Error::Symbol => write!(f, "Namespaced names not matching any macros found in the code."),
      Error::Placeholder => write!(f, "Placeholders shouldn't even occur in the code during macro execution, this is likely a compiler bug"),
      Error::ExprToClause(expr) => write!(f, "Attempted to transform the clause (foo:bar) into a typed clause. This is likely a compiler bug"),
      Error::NonInfixAt => write!(f, "@ as a token can only ever occur between a generic and a type parameter.")
    }
  }
}

/// Try to convert an expression from AST format to typed lambda
pub fn expr(expr: &ast::Expr) -> Result<postmacro::Expr, Error> {
  expr_rec(expr, Context::default())
}

/// Try and convert a single clause from AST format to typed lambda
pub fn clause(clause: &ast::Clause) -> Result<postmacro::Clause, Error> {
  clause_rec(clause, Context::default())
}

/// Try and convert a sequence of expressions from AST format to typed lambda 
pub fn exprv(exprv: &[ast::Expr]) -> Result<postmacro::Expr, Error> {
  exprv_rec(exprv, Context::default())
}

#[derive(Clone, Copy)]
struct Context<'a> {
  names: Stackframe<'a, (&'a str, bool)>
}

impl<'a> Context<'a> {
  fn w_name<'b>(&'b self, name: &'b str, is_auto: bool) -> Context<'b> where 'a: 'b {
    Context { names: self.names.push((name, is_auto)) }
  }
}

impl Default for Context<'static> {
  fn default() -> Self {
    Self { names: Stackframe::new(("", false)) }
  }
}

/// Recursive state of [exprv]
fn exprv_rec<'a>(
  v: &'a [ast::Expr],
  ctx: Context<'a>,
) -> Result<postmacro::Expr, Error> {
  let (last, rest) = v.split_last().ok_or(Error::EmptyS)?;
  if rest.len() == 0 {return expr_rec(&v[0], ctx)}
  let clause = if let ast::Expr(ast::Clause::Explicit(inner), empty_slice) = last {
    assert!(empty_slice.len() == 0,
      "It is assumed that Explicit nodes can never have type annotations as the \
      wrapped expression node matches all trailing colons."
    );
    let x = expr_rec(inner.as_ref(), ctx)?;
    postmacro::Clause::Explicit(Rc::new(exprv_rec(rest, ctx)?), Rc::new(x))
  } else {
    let f = exprv_rec(rest, ctx)?;
    let x = expr_rec(last, ctx)?;
    postmacro::Clause::Apply(Rc::new(f), Rc::new(x))
  };
  Ok(postmacro::Expr(clause, Rc::new(vec![])))
}

/// Recursive state of [expr]
fn expr_rec<'a>(
  ast::Expr(val, typ): &'a ast::Expr,
  ctx: Context<'a>
) -> Result<postmacro::Expr, Error> { // (output, used_explicits)
  let typ: Vec<postmacro::Clause> = typ.iter()
    .map(|c| clause_rec(c, ctx))
    .collect::<Result<_, _>>()?;
  if let ast::Clause::S(paren, body) = val {
    if *paren != '(' {return Err(Error::BadGroup(*paren))}
    let postmacro::Expr(inner, inner_t) = exprv_rec(body.as_ref(), ctx)?;
    let new_t =
      if typ.len() == 0 { inner_t }
      else if inner_t.len() == 0 { Rc::new(typ) }
      else { Rc::new(inner_t.iter().chain(typ.iter()).cloned().collect()) };
    Ok(postmacro::Expr(inner, new_t))
  } else {
    let cls = clause_rec(&val, ctx)?;
    Ok(postmacro::Expr(cls, Rc::new(typ)))
  }
}

// (\t:(@T. Pair T T). t \left.\right. left) @number -- this will fail
// (@T. \t:Pair T T. t \left.\right. left) @number -- this is the correct phrasing

/// Recursive state of [clause]
fn clause_rec<'a>(
  cls: &'a ast::Clause,
  ctx: Context<'a>
) -> Result<postmacro::Clause, Error> {
  match cls {
    ast::Clause::P(p) => Ok(postmacro::Clause::P(p.clone())),
    ast::Clause::Auto(no, t, b) => {
      let typ = if t.len() == 0 {Rc::new(vec![])} else {
        let postmacro::Expr(c, t) = exprv_rec(t.as_ref(), ctx)?;
        if t.len() > 0 {return Err(Error::ExplicitKindOfType)}
        else {Rc::new(vec![c])}
      };
      let body_ctx = if let Some(name) = no {
        ctx.w_name(&&**name, true)
      } else {ctx};
      let body = exprv_rec(b.as_ref(), body_ctx)?;
      Ok(postmacro::Clause::Auto(typ, Rc::new(body)))
    }
    ast::Clause::Lambda(n, t, b) => {
      let typ = if t.len() == 0 {Rc::new(vec![])} else {
        let postmacro::Expr(c, t) = exprv_rec(t.as_ref(), ctx)?;
        if t.len() > 0 {return Err(Error::ExplicitKindOfType)}
        else {Rc::new(vec![c])}
      };
      let body_ctx = ctx.w_name(&&**n, false);
      let body = exprv_rec(b.as_ref(), body_ctx)?;
      Ok(postmacro::Clause::Lambda(typ, Rc::new(body)))
    }
    ast::Clause::Name { local: Some(arg), .. } => {
      let (level, (_, is_auto)) = ctx.names.iter().enumerate().find(|(_, (n, _))| n == arg)
        .ok_or_else(|| Error::Unbound(arg.clone()))?;
      let label = if *is_auto {postmacro::Clause::AutoArg} else {postmacro::Clause::LambdaArg};
      Ok(label(level))
    }
    ast::Clause::S(paren, entries) => {
      if *paren != '(' {return Err(Error::BadGroup(*paren))}
      let postmacro::Expr(val, typ) = exprv_rec(entries.as_ref(), ctx)?;
      if typ.len() == 0 {Ok(val)}
      else {Err(Error::ExprToClause(postmacro::Expr(val, typ)))}
    },
    ast::Clause::Name { local: None, .. } => Err(Error::Symbol),
    ast::Clause::Placeh { .. } => Err(Error::Placeholder),
    ast::Clause::Explicit(..) => Err(Error::NonInfixAt)
  }
}