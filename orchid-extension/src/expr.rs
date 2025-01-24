use std::fmt;
use std::rc::Rc;

use async_once_cell::OnceCell;
use derive_destructure::destructure;
use futures::task::LocalSpawnExt;
use orchid_base::error::OrcErrv;
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;

use crate::api;
use crate::atom::ForeignAtom;
use crate::gen_expr::{GExpr, GExprKind};
use crate::system::SysCtx;

#[derive(destructure)]
pub struct ExprHandle {
	pub tk: api::ExprTicket,
	pub ctx: SysCtx,
}
impl ExprHandle {
	pub(crate) fn from_args(ctx: SysCtx, tk: api::ExprTicket) -> Self { Self { ctx, tk } }
	pub fn get_ctx(&self) -> SysCtx { self.ctx.clone() }
	pub async fn clone(&self) -> Self {
		self.ctx.reqnot.notify(api::Acquire(self.ctx.id, self.tk)).await;
		Self { ctx: self.ctx.clone(), tk: self.tk }
	}
}
impl fmt::Debug for ExprHandle {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "ExprHandle({})", self.tk.0)
	}
}
impl Drop for ExprHandle {
	fn drop(&mut self) {
		let notif = api::Release(self.ctx.id, self.tk);
		let SysCtx { reqnot, spawner, logger, .. } = self.ctx.clone();
		if let Err(e) = spawner.spawn_local(async move { reqnot.notify(notif).await }) {
			writeln!(logger, "Failed to schedule notification about resource release: {e}");
		}
	}
}

#[derive(Clone, Debug, destructure)]
pub struct Expr {
	handle: Rc<ExprHandle>,
	data: Rc<OnceCell<ExprData>>,
}
impl Expr {
	pub fn from_handle(handle: Rc<ExprHandle>) -> Self { Self { handle, data: Rc::default() } }
	pub fn new(handle: Rc<ExprHandle>, d: ExprData) -> Self {
		Self { handle, data: Rc::new(OnceCell::from(d)) }
	}

	pub async fn data(&self) -> &ExprData {
		(self.data.get_or_init(async {
			let details = self.handle.ctx.reqnot.request(api::Inspect { target: self.handle.tk }).await;
			let pos = Pos::from_api(&details.location, &self.handle.ctx.i).await;
			let kind = match details.kind {
				api::InspectedKind::Atom(a) =>
					ExprKind::Atom(ForeignAtom::new(self.handle.clone(), a, pos.clone())),
				api::InspectedKind::Bottom(b) =>
					ExprKind::Bottom(OrcErrv::from_api(&b, &self.handle.ctx.i).await),
				api::InspectedKind::Opaque => ExprKind::Opaque,
			};
			ExprData { pos, kind }
		}))
		.await
	}
	pub async fn atom(self) -> Result<ForeignAtom<'static>, Self> {
		match self.data().await {
			ExprData { kind: ExprKind::Atom(atom), .. } => Ok(atom.clone()),
			_ => Err(self),
		}
	}
	pub fn handle(&self) -> Rc<ExprHandle> { self.handle.clone() }
	pub fn ctx(&self) -> SysCtx { self.handle.ctx.clone() }

	pub fn gen(&self) -> GExpr { GExpr { pos: Pos::SlotTarget, kind: GExprKind::Slot(self.clone()) } }
}

#[derive(Clone, Debug)]
pub struct ExprData {
	pub pos: Pos,
	pub kind: ExprKind,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
	Atom(ForeignAtom<'static>),
	Bottom(OrcErrv),
	Opaque,
}
