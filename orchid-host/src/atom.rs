use std::fmt;
use std::rc::{Rc, Weak};

use derive_destructure::destructure;
use orchid_base::format::{FmtCtx, FmtUnit, Format, take_first_fmt};
use orchid_base::location::Pos;
use orchid_base::reqnot::Requester;
use orchid_base::tree::AtomRepr;

use crate::api;
use crate::ctx::Ctx;
use crate::expr::Expr;
use crate::extension::Extension;
use crate::system::System;

#[derive(destructure)]
pub struct AtomData {
	owner: System,
	drop: Option<api::AtomId>,
	data: Vec<u8>,
}
impl AtomData {
	fn api(self) -> api::Atom {
		let (owner, drop, data) = self.destructure();
		api::Atom { data, drop, owner: owner.id() }
	}
	fn api_ref(&self) -> api::Atom {
		api::Atom { data: self.data.clone(), drop: self.drop, owner: self.owner.id() }
	}
}
impl Drop for AtomData {
	fn drop(&mut self) {
		if let Some(id) = self.drop {
			self.owner.drop_atom(id);
		}
	}
}
impl fmt::Debug for AtomData {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("AtomData")
			.field("drop", &self.drop)
			.field("data", &self.data)
			.field("owner", &self.owner.id())
			.finish()
	}
}

#[derive(Clone, Debug)]
pub struct AtomHand(Rc<AtomData>);
impl AtomHand {
	pub(crate) async fn new(api::Atom { data, drop, owner }: api::Atom, ctx: &Ctx) -> Self {
		let create = || async {
			let owner = ctx.system_inst(owner).await.expect("Dropped system created atom");
			AtomHand(Rc::new(AtomData { data, owner, drop }))
		};
		if let Some(id) = drop {
			let mut owned_g = ctx.owned_atoms.write().await;
			if let Some(data) = owned_g.get(&id) {
				if let Some(atom) = data.upgrade() {
					return atom;
				}
			}
			let new = create().await;
			owned_g.insert(id, new.downgrade());
			new
		} else {
			create().await
		}
	}
	pub async fn call(self, arg: Expr) -> api::Expression {
		let owner_sys = self.0.owner.clone();
		let reqnot = owner_sys.reqnot();
		owner_sys.ext().exprs().give_expr(arg.clone());
		match Rc::try_unwrap(self.0) {
			Ok(data) => reqnot.request(api::FinalCall(data.api(), arg.id())).await,
			Err(hand) => reqnot.request(api::CallRef(hand.api_ref(), arg.id())).await,
		}
	}
	pub fn sys(&self) -> &System { &self.0.owner }
	pub fn ext(&self) -> &Extension { self.sys().ext() }
	pub async fn req(&self, key: api::TStrv, req: Vec<u8>) -> Option<Vec<u8>> {
		self.0.owner.reqnot().request(api::Fwded(self.0.api_ref(), key, req)).await
	}
	pub fn api_ref(&self) -> api::Atom { self.0.api_ref() }
	pub async fn to_string(&self) -> String { take_first_fmt(self, &self.0.owner.ctx().i).await }
	pub fn downgrade(&self) -> WeakAtomHand { WeakAtomHand(Rc::downgrade(&self.0)) }
}
impl Format for AtomHand {
	async fn print<'a>(&'a self, _c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		FmtUnit::from_api(&self.0.owner.reqnot().request(api::AtomPrint(self.0.api_ref())).await)
	}
}
impl AtomRepr for AtomHand {
	type Ctx = Ctx;
	async fn from_api(atom: &orchid_api::Atom, _: Pos, ctx: &mut Self::Ctx) -> Self {
		Self::new(atom.clone(), ctx).await
	}
	async fn to_api(&self) -> orchid_api::Atom { self.api_ref() }
}

pub struct WeakAtomHand(Weak<AtomData>);
impl WeakAtomHand {
	pub fn upgrade(&self) -> Option<AtomHand> { self.0.upgrade().map(AtomHand) }
}
