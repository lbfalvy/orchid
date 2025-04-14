use std::mem;

use async_std::sync::RwLockWriteGuard;
use bound::Bound;
use futures::FutureExt;
use orchid_base::error::OrcErrv;
use orchid_base::format::{FmtCtxImpl, Format, take_first};
use orchid_base::location::Pos;
use orchid_base::logging::Logger;

use crate::ctx::Ctx;
use crate::expr::{Expr, ExprKind, ExprParseCtx, PathSet, PathSetBuilder, Step};

type ExprGuard = Bound<RwLockWriteGuard<'static, ExprKind>, Expr>;

/// The stack operation associated with a transform
enum StackOp {
	Pop,
	Nop,
	Push(Expr),
	Swap(Expr),
	Unwind(OrcErrv),
}

pub enum ExecResult {
	Value(Expr),
	Gas(ExecCtx),
	Err(OrcErrv),
}

pub struct ExecCtx {
	ctx: Ctx,
	gas: Option<u64>,
	stack: Vec<ExprGuard>,
	cur: ExprGuard,
	cur_pos: Pos,
	did_pop: bool,
	logger: Logger,
}
impl ExecCtx {
	pub async fn new(ctx: Ctx, logger: Logger, init: Expr) -> Self {
		let cur_pos = init.pos();
		let cur = Bound::async_new(init, |init| init.kind().write()).await;
		Self { ctx, gas: None, stack: vec![], cur, cur_pos, did_pop: false, logger }
	}
	pub fn remaining_gas(&self) -> u64 { self.gas.expect("queried remaining_gas but no gas was set") }
	pub fn set_gas(&mut self, gas: Option<u64>) { self.gas = gas }
	pub fn idle(&self) -> bool { self.did_pop }
	pub fn result(self) -> ExecResult {
		if self.idle() {
			match &*self.cur {
				ExprKind::Bottom(errv) => ExecResult::Err(errv.clone()),
				_ => ExecResult::Value(*self.cur.unbind()),
			}
		} else {
			ExecResult::Gas(self)
		}
	}
	pub fn use_gas(&mut self, amount: u64) -> bool {
		if let Some(gas) = &mut self.gas {
			*gas -= amount;
		}
		self.gas != Some(0)
	}
	pub async fn try_lock(&self, ex: &Expr) -> ExprGuard {
		Bound::async_new(ex.clone(), |ex| ex.kind().write()).await
	}
	pub async fn unpack_ident(&self, ex: &Expr) -> Expr {
		match ex.kind().try_write().as_deref_mut() {
			Some(ExprKind::Identity(ex)) => {
				let val = self.unpack_ident(ex).boxed_local().await;
				*ex = val.clone();
				val
			},
			Some(_) => ex.clone(),
			None => panic!("Cycle encountered!"),
		}
	}
	pub async fn execute(&mut self) {
		while self.use_gas(1) {
			let mut kind_swap = ExprKind::Missing;
			mem::swap(&mut kind_swap, &mut self.cur);
			let unit = kind_swap.print(&FmtCtxImpl { i: &self.ctx.i }).await;
			writeln!(self.logger, "Exxecute lvl{} {}", self.stack.len(), take_first(&unit, true));
			let (kind, op) = match kind_swap {
				ExprKind::Identity(target) => {
					let inner = self.unpack_ident(&target).await;
					(ExprKind::Identity(inner.clone()), StackOp::Swap(inner))
				},
				ExprKind::Seq(a, b) if !self.did_pop => (ExprKind::Seq(a.clone(), b), StackOp::Push(a)),
				ExprKind::Seq(_, b) => (ExprKind::Identity(b), StackOp::Nop),
				ExprKind::Const(name) => {
					let root = (self.ctx.root.get().and_then(|v| v.upgrade()))
						.expect("Root not assigned before execute call");
					match root.get_const_value(name, self.cur_pos.clone(), self.ctx.clone()).await {
						Err(e) => (ExprKind::Bottom(e), StackOp::Pop),
						Ok(v) => (ExprKind::Identity(v), StackOp::Nop),
					}
				},
				ExprKind::Arg => panic!("This should not appear outside function bodies"),
				ek @ ExprKind::Atom(_) => (ek, StackOp::Pop),
				ExprKind::Bottom(bot) => (ExprKind::Bottom(bot.clone()), StackOp::Unwind(bot)),
				ExprKind::Call(f, x) if !self.did_pop => (ExprKind::Call(f.clone(), x), StackOp::Push(f)),
				ExprKind::Call(f, x) => match f.try_into_owned_atom().await {
					Ok(atom) => {
						let ext = atom.sys().ext().clone();
						let x_norm = self.unpack_ident(&x).await;
						let mut parse_ctx = ExprParseCtx { ctx: self.ctx.clone(), exprs: ext.exprs().clone() };
						let val =
							Expr::from_api(&atom.call(x_norm).await, PathSetBuilder::new(), &mut parse_ctx).await;
						(ExprKind::Identity(val.clone()), StackOp::Swap(val))
					},
					Err(f) => match &*f.kind().read().await {
						ExprKind::Arg | ExprKind::Call(..) | ExprKind::Seq(..) | ExprKind::Const(_) =>
							panic!("This should not appear outside function bodies"),
						ExprKind::Missing => panic!("Should have been replaced"),
						ExprKind::Atom(a) => {
							let ext = a.sys().ext().clone();
							let x_norm = self.unpack_ident(&x).await;
							let mut parse_ctx =
								ExprParseCtx { ctx: ext.ctx().clone(), exprs: ext.exprs().clone() };
							let val = Expr::from_api(
								&a.clone().call(x_norm).await,
								PathSetBuilder::new(),
								&mut parse_ctx,
							)
							.await;
							(ExprKind::Identity(val.clone()), StackOp::Swap(val))
						},
						ExprKind::Bottom(exprv) => (ExprKind::Bottom(exprv.clone()), StackOp::Pop),
						ExprKind::Lambda(None, body) =>
							(ExprKind::Identity(body.clone()), StackOp::Swap(body.clone())),
						ExprKind::Lambda(Some(path), body) => {
							let output = substitute(body, &path.steps, path.next(), x).await;
							(ExprKind::Identity(output.clone()), StackOp::Swap(output))
						},
						ExprKind::Identity(f) => (ExprKind::Call(f.clone(), x.clone()), StackOp::Nop),
					},
				},
				l @ ExprKind::Lambda(..) => (l, StackOp::Pop),
				ExprKind::Missing => panic!("Should have been replaced"),
			};
			self.did_pop = matches!(op, StackOp::Pop | StackOp::Unwind(_));
			*self.cur = kind;
			match op {
				StackOp::Nop => (),
				StackOp::Pop => match self.stack.pop() {
					Some(top) => self.cur = top,
					None => return,
				},
				StackOp::Push(sub) => {
					self.cur_pos = sub.pos();
					let mut new_guard = self.try_lock(&sub).await;
					mem::swap(&mut self.cur, &mut new_guard);
					self.stack.push(new_guard);
				},
				StackOp::Swap(new) => self.cur = self.try_lock(&new).await,
				StackOp::Unwind(err) => {
					for dependent in self.stack.iter_mut() {
						**dependent = ExprKind::Bottom(err.clone());
					}
					*self.cur = ExprKind::Bottom(err.clone());
					self.stack = vec![];
					return;
				},
			}
		}
	}
}

async fn substitute(
	src: &Expr,
	path: &[Step],
	next: Option<(&PathSet, &PathSet)>,
	val: Expr,
) -> Expr {
	let exk = src.kind().try_read().expect("Cloned function body parts must never be written");
	let kind = match (&*exk, path.split_first()) {
		(ExprKind::Identity(x), _) => return substitute(x, path, next, val).boxed_local().await,
		(ExprKind::Lambda(ps, b), _) =>
			ExprKind::Lambda(ps.clone(), substitute(b, path, next, val).boxed_local().await),
		(exk, None) => match (exk, next) {
			(ExprKind::Arg, None) => return val.clone(),
			(ExprKind::Call(f, x), Some((l, r))) => ExprKind::Call(
				substitute(f, &l.steps, l.next(), val.clone()).boxed_local().await,
				substitute(x, &r.steps, r.next(), val.clone()).boxed_local().await,
			),
			(ExprKind::Seq(a, b), Some((l, r))) => ExprKind::Seq(
				substitute(a, &l.steps, l.next(), val.clone()).boxed_local().await,
				substitute(b, &r.steps, r.next(), val.clone()).boxed_local().await,
			),
			(_, None) => panic!("Can only substitute Arg"),
			(_, Some(_)) => panic!("Can only fork into Call and Seq"),
		},
		(ExprKind::Call(f, x), Some((Step::Left, tail))) =>
			ExprKind::Call(substitute(f, tail, next, val).boxed_local().await, x.clone()),
		(ExprKind::Call(f, x), Some((Step::Right, tail))) =>
			ExprKind::Call(f.clone(), substitute(x, tail, next, val).boxed_local().await),
		(ExprKind::Seq(f, x), Some((Step::Left, tail))) =>
			ExprKind::Seq(substitute(f, tail, next, val).boxed_local().await, x.clone()),
		(ExprKind::Seq(f, x), Some((Step::Right, tail))) =>
			ExprKind::Seq(f.clone(), substitute(x, tail, next, val).boxed_local().await),
		(ek, Some(_)) => panic!("Path leads into {ek:?}"),
	};
	kind.at(src.pos())
}
