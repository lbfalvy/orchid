use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use itertools::Itertools;
use lazy_static::lazy_static;
use orchid_api_traits::Encode;
use orchid_base::error::OrcRes;
use orchid_base::interner::Tok;
use orchid_base::name::Sym;
use trait_set::trait_set;

use crate::atom::{MethodSet, Atomic};
use crate::atom_owned::{DeserializeCtx, OwnedAtom, OwnedVariant};
use crate::conv::ToExpr;
use crate::expr::{Expr, ExprHandle};
use crate::system::SysCtx;

trait_set! {
  trait FunCB = Fn(Vec<Expr>) -> OrcRes<Expr> + Send + Sync + 'static;
}

pub trait ExprFunc<I, O>: Clone + Send + Sync + 'static {
  const ARITY: u8;
  fn apply(&self, v: Vec<Expr>) -> OrcRes<Expr>;
}

lazy_static! {
  static ref FUNS: Mutex<HashMap<Sym, (u8, Arc<dyn FunCB>)>> = Mutex::default();
}

#[derive(Clone)]
pub(crate) struct Fun {
  path: Sym,
  args: Vec<Expr>,
  arity: u8,
  fun: Arc<dyn FunCB>,
}
impl Fun {
  pub fn new<I, O, F: ExprFunc<I, O>>(path: Sym, f: F) -> Self {
    let mut fung = FUNS.lock().unwrap();
    let fun = if let Some(x) = fung.get(&path) {
      x.1.clone()
    } else {
      let fun = Arc::new(move |v| f.apply(v));
      fung.insert(path.clone(), (F::ARITY, fun.clone()));
      fun
    };
    Self { args: vec![], arity: F::ARITY, path, fun }
  }
}
impl Atomic for Fun {
  type Data = ();
  type Variant = OwnedVariant;
  fn reg_reqs() -> MethodSet<Self> { MethodSet::new() }
}
impl OwnedAtom for Fun {
  type Refs = Vec<Expr>;
  fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
  fn call_ref(&self, arg: ExprHandle) -> Expr {
    let new_args = self.args.iter().cloned().chain([Expr::new(Arc::new(arg))]).collect_vec();
    if new_args.len() == self.arity.into() {
      (self.fun)(new_args).to_expr()
    } else {
      Self { args: new_args, arity: self.arity, fun: self.fun.clone(), path: self.path.clone() }
        .to_expr()
    }
  }
  fn call(self, arg: ExprHandle) -> Expr { self.call_ref(arg) }
  fn serialize(&self, _: SysCtx, sink: &mut (impl io::Write + ?Sized)) -> Self::Refs {
    self.path.encode(sink);
    self.args.clone()
  }
  fn deserialize(ctx: impl DeserializeCtx, args: Self::Refs) -> Self {
    let path = Sym::new(ctx.decode::<Vec<Tok<String>>>()).unwrap();
    let (arity, fun) = FUNS.lock().unwrap().get(&path).unwrap().clone();
    Self { args, arity, path, fun }
  }
}

mod expr_func_derives {
  use orchid_base::error::OrcRes;

  use super::ExprFunc;
  use crate::conv::{ToExpr, TryFromExpr};
  use crate::func_atom::Expr;

  macro_rules! expr_func_derive {
    ($arity: tt, $($t:ident),*) => {
      paste::paste!{
        impl<
          $($t: TryFromExpr, )*
          Out: ToExpr,
          Func: Fn($($t,)*) -> Out + Clone + Send + Sync + 'static
        > ExprFunc<($($t,)*), Out> for Func {
          const ARITY: u8 = $arity;
          fn apply(&self, v: Vec<Expr>) -> OrcRes<Expr> {
            assert_eq!(v.len(), Self::ARITY.into(), "Arity mismatch");
            let [$([< $t:lower >],)*] = v.try_into().unwrap_or_else(|_| panic!("Checked above"));
            Ok(self($($t::try_from_expr([< $t:lower >])?,)*).to_expr())
          }
        }
      }
    };
  }
  expr_func_derive!(1, A);
  expr_func_derive!(2, A, B);
  expr_func_derive!(3, A, B, C);
  expr_func_derive!(4, A, B, C, D);
  expr_func_derive!(5, A, B, C, D, E);
  expr_func_derive!(6, A, B, C, D, E, F);
  expr_func_derive!(7, A, B, C, D, E, F, G);
  expr_func_derive!(8, A, B, C, D, E, F, G, H);
  expr_func_derive!(9, A, B, C, D, E, F, G, H, I);
  expr_func_derive!(10, A, B, C, D, E, F, G, H, I, J);
  expr_func_derive!(11, A, B, C, D, E, F, G, H, I, J, K);
  expr_func_derive!(12, A, B, C, D, E, F, G, H, I, J, K, L);
  expr_func_derive!(13, A, B, C, D, E, F, G, H, I, J, K, L, M);
  expr_func_derive!(14, A, B, C, D, E, F, G, H, I, J, K, L, M, N);
}
