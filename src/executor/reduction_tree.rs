use mappable_rc::Mrc;

use crate::box_chain;
use crate::utils::BoxedIter;
use crate::utils::iter::{box_once, box_empty};

use super::apply_lambda::apply_lambda;
use super::super::representations::typed::{Clause, Expr};

/// Call the function with the first Expression that isn't an Auto,
/// wrap all elements in the returned iterator back in the original sequence of Autos.
pub fn skip_autos<'a,
    F: 'a + FnOnce(Mrc<Expr>, usize) -> I,
    I: Iterator<Item = Mrc<Expr>> + 'static
>(
    depth: usize, expr: Mrc<Expr>, function: F
) -> BoxedIter<'static, Mrc<Expr>> {
    match expr.as_ref() {
        Expr(Clause::Auto(arg, body), typ) => {
            return Box::new(skip_autos(depth + 1, Mrc::clone(body), function).map({
                let arg = arg.as_ref().map(Mrc::clone);
                let typ = Mrc::clone(typ);
                move |body| {
                    Mrc::new(Expr(Clause::Auto(
                        arg.as_ref().map(Mrc::clone),
                        body
                    ), Mrc::clone(&typ)))
                }
            })) as BoxedIter<'static, Mrc<Expr>>
        }
        Expr(Clause::Explicit(expr, targ), typ) => {
            return Box::new(skip_autos(depth, Mrc::clone(expr), function).map({
                let (targ, typ) = (Mrc::clone(targ), Mrc::clone(typ));
                move |expr| {
                    Mrc::new(Expr(Clause::Explicit(expr, Mrc::clone(&targ)), Mrc::clone(&typ)))
                }
            })) as BoxedIter<'static, Mrc<Expr>>
        }
        _ => ()
    }
    Box::new(function(expr, depth))
}

/// Produces an iterator of every expression that can be produced from this one through B-reduction.
fn direct_reductions(ex: Mrc<Expr>) -> impl Iterator<Item = Mrc<Expr>> {
    skip_autos(0, ex, |mexpr, _| {
        let Expr(clause, typ_ref) = mexpr.as_ref();
        match clause {
            Clause::Apply(f, x) => box_chain!(
                skip_autos(0, Mrc::clone(f), |mexpr, _| {
                    let Expr(f, _) = mexpr.as_ref();
                    match f {
                        Clause::Lambda(_, body) => box_once(
                            apply_lambda(Mrc::clone(body), Mrc::clone(x))
                        ),
                        Clause::ExternFn(xfn) => {
                            let Expr(xval, xtyp) = x.as_ref();
                            xfn.apply(xval.clone())
                                .map(|ret| box_once(Mrc::new(Expr(ret, Mrc::clone(xtyp)))))
                                .unwrap_or(box_empty())
                        },
                        // Parametric newtypes are atoms of function type
                        Clause::Atom(..) | Clause::Argument(..) | Clause::Apply(..) => box_empty(),
                        Clause::Literal(lit) => 
                            panic!("Literal expression {lit:?} can't be applied as function"),
                        Clause::Auto(..) | Clause::Explicit(..) => 
                            unreachable!("skip_autos should have filtered these"),
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
            Clause::Lambda(argt, body) => Box::new(direct_reductions(Mrc::clone(body)).map({
                let typ = Mrc::clone(typ_ref);
                let argt = argt.as_ref().map(Mrc::clone);
                move |body| Mrc::new(Expr(Clause::Lambda(
                    argt.as_ref().map(Mrc::clone),
                    body
                ), Mrc::clone(&typ)))
            })),
            Clause::Literal(..) | Clause::ExternFn(..) | Clause::Atom(..) | Clause::Argument(..) =>
                box_empty(),
            Clause::Auto(..) | Clause::Explicit(..) =>
                unreachable!("skip_autos should have filtered these"),
        }
    })
}

