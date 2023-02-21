use mappable_rc::Mrc;

use crate::box_chain;
use crate::utils::BoxedIter;
use crate::utils::iter::{box_once, box_empty};

use super::apply_lambda::apply_lambda;
use super::super::representations::typed::{Clause, Expr};

/// Call the function with the first Expression that isn't an Auto,
/// wrap all elements in the returned iterator back in the original sequence of Autos.
pub fn skip_autos<'a,
  F: 'a + FnOnce(Mrc<Expr>) -> I,
  I: Iterator<Item = Mrc<Expr>> + 'static
>(
  expr: Mrc<Expr>, function: F
) -> BoxedIter<'static, Mrc<Expr>> {
  if let Expr(Clause::Auto(id, arg, body), typ) = expr.as_ref() {
    return Box::new(skip_autos(Mrc::clone(body), function).map({
      let arg = Mrc::clone(arg);
      let typ = Mrc::clone(typ);
      move |body| {
        Mrc::new(Expr(Clause::Auto(
          *id, 
          Mrc::clone(&arg),
          body
        ), Mrc::clone(&typ)))
      }
    })) as BoxedIter<'static, Mrc<Expr>>
  }
  Box::new(function(expr))
}

/// Produces an iterator of every expression that can be produced from this one through B-reduction.
fn direct_reductions(ex: Mrc<Expr>) -> impl Iterator<Item = Mrc<Expr>> {
  skip_autos(ex, |mexpr| {
    let Expr(clause, typ_ref) = mexpr.as_ref();
    match clause {
      Clause::Apply(f, x) => box_chain!(
        skip_autos(Mrc::clone(f), |mexpr| {
          let Expr(f, _) = mexpr.as_ref();
          match f {
            Clause::Lambda(id, _, body) => box_once(
              apply_lambda(*id, Mrc::clone(x), Mrc::clone(body))
            ),
            Clause::ExternFn(xfn) => {
              let Expr(xval, xtyp) = x.as_ref();
              xfn.apply(xval.clone())
                .map(|ret| box_once(Mrc::new(Expr(ret, Mrc::clone(xtyp)))))
                .unwrap_or(box_empty())
            },
            // Parametric newtypes are atoms of function type
            Clause::Atom(..) | Clause::LambdaArg(..) | Clause::AutoArg(..) | Clause::Apply(..) => box_empty(),
            Clause::Literal(lit) => 
              panic!("Literal expression {lit:?} can't be applied as function"),
            Clause::Auto(..) => unreachable!("skip_autos should have filtered this"),
          }
        }),
        direct_reductions(Mrc::clone(f)).map({
          let typ = Mrc::clone(typ_ref);
          let x = Mrc::clone(x);
          move |f| Mrc::new(Expr(Clause::Apply(
            f,
            Mrc::clone(&x)
          ), Mrc::clone(&typ)))
        }),
        direct_reductions(Mrc::clone(x)).map({
          let typ = Mrc::clone(typ_ref);
          let f = Mrc::clone(f);
          move |x| Mrc::new(Expr(Clause::Apply(
            Mrc::clone(&f),
            x
          ), Mrc::clone(&typ)))
        })
      ),
      Clause::Lambda(id, argt, body) => {
        let id = *id;
        let typ = Mrc::clone(typ_ref);
        let argt = Mrc::clone(argt);
        let body = Mrc::clone(body);
        let body_reductions = direct_reductions(body)
          .map(move |body| {
            let argt = Mrc::clone(&argt);
            Mrc::new(Expr(
              Clause::Lambda(id, argt, body),
              Mrc::clone(&typ)
            ))
          });
        Box::new(body_reductions)
      },
      Clause::Auto(..) => unreachable!("skip_autos should have filtered this"),
      Clause::Literal(..) | Clause::ExternFn(..) | Clause::Atom(..)
      | Clause::LambdaArg(..) | Clause::AutoArg(..) => box_empty(),
    }
  })
}

/*



 */