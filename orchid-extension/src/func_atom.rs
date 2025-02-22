use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use async_std::io::Write;
use async_std::sync::Mutex;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use itertools::Itertools;
use never::Never;
use orchid_api_traits::Encode;
use orchid_base::clone;
use orchid_base::error::OrcRes;
use orchid_base::name::Sym;
use trait_set::trait_set;

use crate::atom::{Atomic, MethodSetBuilder};
use crate::atom_owned::{DeserializeCtx, OwnedAtom, OwnedVariant};
use crate::conv::ToExpr;
use crate::expr::{Expr, ExprHandle};
use crate::gen_expr::GExpr;
use crate::system::{SysCtx, SysCtxEntry};

trait_set! {
	trait FunCB = Fn(Vec<Expr>) -> LocalBoxFuture<'static, OrcRes<GExpr>> + 'static;
}

pub trait ExprFunc<I, O>: Clone + 'static {
	const ARITY: u8;
	fn apply(&self, v: Vec<Expr>) -> impl Future<Output = OrcRes<GExpr>>;
}

#[derive(Default)]
struct FunsCtx(Mutex<HashMap<Sym, (u8, Rc<dyn FunCB>)>>);
impl SysCtxEntry for FunsCtx {}

/// An Atom representing a partially applied named native function. These
/// partial calls are serialized into the name of the native function and the
/// argument list.
///
/// See [Lambda] for the non-serializable variant
#[derive(Clone)]
pub(crate) struct Fun {
	path: Sym,
	args: Vec<Expr>,
	arity: u8,
	fun: Rc<dyn FunCB>,
}
impl Fun {
	pub async fn new<I, O, F: ExprFunc<I, O>>(path: Sym, ctx: SysCtx, f: F) -> Self {
		let funs: &FunsCtx = ctx.get_or_default();
		let mut fung = funs.0.lock().await;
		let fun = if let Some(x) = fung.get(&path) {
			x.1.clone()
		} else {
			let fun = Rc::new(move |v| clone!(f; async move { f.apply(v).await }.boxed_local()));
			fung.insert(path.clone(), (F::ARITY, fun.clone()));
			fun
		};
		Self { args: vec![], arity: F::ARITY, path, fun }
	}
	pub fn arity(&self) -> u8 { self.arity }
}
impl Atomic for Fun {
	type Data = ();
	type Variant = OwnedVariant;
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new() }
}
impl OwnedAtom for Fun {
	type Refs = Vec<Expr>;
	async fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
	async fn call_ref(&self, arg: ExprHandle) -> GExpr {
		std::io::Write::flush(&mut std::io::stderr()).unwrap();
		let new_args = self.args.iter().cloned().chain([Expr::from_handle(Rc::new(arg))]).collect_vec();
		if new_args.len() == self.arity.into() {
			(self.fun)(new_args).await.to_expr()
		} else {
			Self { args: new_args, arity: self.arity, fun: self.fun.clone(), path: self.path.clone() }
				.to_expr()
		}
	}
	async fn call(self, arg: ExprHandle) -> GExpr { self.call_ref(arg).await }
	async fn serialize(&self, _: SysCtx, write: Pin<&mut (impl Write + ?Sized)>) -> Self::Refs {
		self.path.to_api().encode(write).await;
		self.args.clone()
	}
	async fn deserialize(mut ctx: impl DeserializeCtx, args: Self::Refs) -> Self {
		let sys = ctx.sys();
		let path = Sym::from_api(ctx.decode().await, sys.i()).await;
		let (arity, fun) = sys.get_or_default::<FunsCtx>().0.lock().await.get(&path).unwrap().clone();
		Self { args, arity, path, fun }
	}
	async fn print(&self, _: SysCtx) -> orchid_base::format::FmtUnit {
		format!("{}:{}/{}", self.path, self.args.len(), self.arity).into()
	}
}

/// An Atom representing a partially applied native lambda. These are not
/// serializable.
///
/// See [Fun] for the serializable variant
#[derive(Clone)]
pub struct Lambda {
	args: Vec<Expr>,
	arity: u8,
	fun: Rc<dyn FunCB>,
}
impl Lambda {
	pub fn new<I, O, F: ExprFunc<I, O>>(f: F) -> Self {
		let fun = Rc::new(move |v| clone!(f; async move { f.apply(v).await }.boxed_local()));
		Self { args: vec![], arity: F::ARITY, fun }
	}
}
impl Atomic for Lambda {
	type Data = ();
	type Variant = OwnedVariant;
	fn reg_reqs() -> MethodSetBuilder<Self> { MethodSetBuilder::new() }
}
impl OwnedAtom for Lambda {
	type Refs = Never;
	async fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
	async fn call_ref(&self, arg: ExprHandle) -> GExpr {
		let new_args = self.args.iter().cloned().chain([Expr::from_handle(Rc::new(arg))]).collect_vec();
		if new_args.len() == self.arity.into() {
			(self.fun)(new_args).await.to_expr()
		} else {
			Self { args: new_args, arity: self.arity, fun: self.fun.clone() }.to_expr()
		}
	}
	async fn call(self, arg: ExprHandle) -> GExpr { self.call_ref(arg).await }
}

mod expr_func_derives {
	use orchid_base::error::OrcRes;

	use super::ExprFunc;
	use crate::conv::{ToExpr, TryFromExpr};
	use crate::func_atom::Expr;
	use crate::gen_expr::GExpr;

	macro_rules! expr_func_derive {
    ($arity: tt, $($t:ident),*) => {
      paste::paste!{
        impl<
          $($t: TryFromExpr, )*
					Out: ToExpr,
          Func: AsyncFn($($t,)*) -> Out + Clone + Send + Sync + 'static
        > ExprFunc<($($t,)*), Out> for Func {
          const ARITY: u8 = $arity;
          async fn apply(&self, v: Vec<Expr>) -> OrcRes<GExpr> {
            assert_eq!(v.len(), Self::ARITY.into(), "Arity mismatch");
            let [$([< $t:lower >],)*] = v.try_into().unwrap_or_else(|_| panic!("Checked above"));
            Ok(self($($t::try_from_expr([< $t:lower >]).await?,)*).await.to_expr())
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
