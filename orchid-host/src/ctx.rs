use std::cell::RefCell;
use std::num::NonZeroU16;
use std::rc::Rc;
use std::{fmt, ops};

use async_std::sync::RwLock;
use futures::task::LocalSpawn;
use hashbrown::HashMap;
use orchid_api::SysId;
use orchid_base::interner::Interner;

use crate::api;
use crate::atom::WeakAtomHand;
use crate::system::{System, WeakSystem};

pub struct CtxData {
	pub i: Rc<Interner>,
	pub spawn: Rc<dyn LocalSpawn>,
	pub systems: RwLock<HashMap<api::SysId, WeakSystem>>,
	pub system_id: RefCell<NonZeroU16>,
	pub owned_atoms: RwLock<HashMap<api::AtomId, WeakAtomHand>>,
}
#[derive(Clone)]
pub struct Ctx(Rc<CtxData>);
impl ops::Deref for Ctx {
	type Target = CtxData;
	fn deref(&self) -> &Self::Target { &*self.0 }
}
impl Ctx {
	pub(crate) async fn system_inst(&self, id: api::SysId) -> Option<System> {
		self.systems.read().await.get(&id).and_then(WeakSystem::upgrade)
	}
	pub(crate) fn next_sys_id(&self) -> api::SysId {
		let mut g = self.system_id.borrow_mut();
		*g = g.checked_add(1).unwrap_or(NonZeroU16::new(1).unwrap());
		SysId(*g)
	}
}
impl fmt::Debug for Ctx {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Ctx")
			.field("i", &self.i)
			.field("system_id", &self.system_id)
			.finish_non_exhaustive()
	}
}
