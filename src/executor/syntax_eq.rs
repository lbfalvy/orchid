use std::collections::HashMap;

use itertools::Itertools;
use mappable_rc::Mrc;

use crate::utils::{ProtoMap, Side, mrc_empty_slice, collect_to_mrc, Stackframe, mrc_concat, Product2};

use super::super::representations::typed::{Clause, Expr};

pub fn swap<T, U>((t, u): (T, U)) -> (U, T) { (u, t) }

// @ @ (0, (foo 1)) ~ @ (0, 0)

// TODO:
// - get rid of leftovers from Explicit
// - adapt to new index-based system

enum UnifError {
  Conflict,
}

type LambdaMap<'a> = Option<&'a Stackframe<'a, (u64, u64)>>;

/// The context associates a given variable (by absolute index) on a given side to
/// an expression on the opposite side rooted at the specified depth.
/// The root depths are used to translate betwee de Brujin arguments and absolute indices.
struct Context(HashMap<u64, Mrc<Expr>>);
impl Context {
  fn set(&mut self, id: u64, value: &Mrc<Expr>, lambdas: LambdaMap) -> Result<Option<Mrc<Expr>>, UnifError> {
    Ok(
      if let Some(local) = self.0.get(&id) {
        Some(
          self.unify_expr(local, value, lambdas)?
          .pick(Mrc::clone(local), Mrc::clone(value))
        )
      } else { None }
    )
  }
  
  fn unify_expr(&mut self,
    left: &Mrc<Expr>, right: &Mrc<Expr>, lambdas: LambdaMap
  ) -> Result<Product2<Mrc<Expr>>, UnifError> {
    let Expr(left_val, left_typs) = left.as_ref();
    let Expr(right_val, right_typs) = right.as_ref();
    let val = match (left_val, right_val) {
      (Clause::AutoArg(l), Clause::AutoArg(r)) if l == r => Product2::Either,
      (Clause::AutoArg(id), _) => self.set(*id, left, lambdas)?.as_ref()
        .map_or(Product2::Left, |e| Product2::New(e.0.clone())),
      (_, Clause::AutoArg(id)) => self.set(*id, right, lambdas)?.as_ref()
        .map_or(Product2::Right, |e| Product2::New(e.0.clone())),
      _ => self.unify_clause(left_val, right_val, lambdas)?
    };
    Ok(match val {
      Product2::Either if right_typs.is_empty() && left_typs.is_empty() => Product2::Either,
      Product2::Left | Product2::Either if right_typs.is_empty() => Product2::Left,
      Product2::Right | Product2::Either if left_typs.is_empty() => Product2::Right,
      product => {
        let all_types = mrc_concat(left_typs, right_typs);
        Product2::New(Mrc::new(Expr(
          product.pick(left_val.clone(), right_val.clone()),
          all_types
        )))
      }
    })
  }

  fn unify_clauses(&mut self,
    left: &Mrc<[Clause]>, right: &Mrc<[Clause]>, lambdas: LambdaMap
  ) -> Result<Product2<Clause>, UnifError> {
    if left.len() != right.len() {return Err(UnifError::Conflict)}
  }

  fn unify_clause(&mut self,
    left: &Clause, right: &Clause, lambdas: LambdaMap
  ) -> Result<Product2<Clause>, UnifError> {
    Ok(match (left, right) {
      (Clause::Literal(l), Clause::Literal(r)) if l == r => Product2::Either,
      (Clause::Atom(l), Clause::Atom(r)) if l == r => Product2::Either,
      (Clause::ExternFn(l), Clause::ExternFn(r)) if l == r => Product2::Either,
      (Clause::LambdaArg(l), Clause::LambdaArg(r)) => if l == r {Product2::Either} else {
        let is_equal = Stackframe::o_into_iter(lambdas)
          .first_some(|(l_candidate, r_candidate)| {
            if l_candidate == l && r_candidate == r {Some(true)} // match
            else if l_candidate == l || r_candidate == r {Some(false)} // shadow
            else {None} // irrelevant
          }).unwrap_or(false);
        // Reference: 
        if is_equal {Product2::Left} else {return Err(UnifError::Conflict)}
      }
      (Clause::AutoArg(_), _) | (_, Clause::AutoArg(_)) => {
        unreachable!("unify_expr should have handled this")
      }
      (Clause::Lambda(l_id, l_arg, l_body), Clause::Lambda(r_id, r_arg, r_body)) => {
        let lambdas = Stackframe::opush(lambdas, (*l_id, *r_id));
        self.unify_expr(l_body, r_body, Some(&lambdas))?
          .map(|ex| Clause::Lambda(*l_id, mrc_empty_slice(), ex))
      }
      (Clause::Apply(l_f, l_x), Clause::Apply(r_f, r_x)) => {
        self.unify_expr(l_f, r_f, lambdas)?.join((Mrc::clone(l_f), Mrc::clone(r_f)), 
          self.unify_expr(l_x, r_x, lambdas)?, (Mrc::clone(l_x), Mrc::clone(r_x))
        ).map(|(f, x)| Clause::Apply(f, x))
      }
      (Clause::Auto(l_id, l_arg, l_body), Clause::Auto(r_id, r_arg, r_body)) => {
        let typ = self.unify(l_arg, r_arg, lambdas)?;
        let body = self.unify_expr(l_body, r_body, lambdas)?;
        typ.join((l_arg, r_arg), )
      }
    })
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