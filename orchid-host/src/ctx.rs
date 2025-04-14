use std::cell::RefCell;
use std::num::{NonZero, NonZeroU16};
use std::rc::{Rc, Weak};
use std::{fmt, ops};

use async_once_cell::OnceCell;
use async_std::sync::RwLock;
use hashbrown::HashMap;
use orchid_api::SysId;
use orchid_base::builtin::Spawner;
use orchid_base::interner::Interner;

use crate::api;
use crate::atom::WeakAtomHand;
use crate::expr_store::ExprStore;
use crate::parsed::Root;
use crate::system::{System, WeakSystem};

pub struct CtxData {
	pub i: Rc<Interner>,
	pub spawn: Spawner,
	pub systems: RwLock<HashMap<api::SysId, WeakSystem>>,
	pub system_id: RefCell<NonZeroU16>,
	pub owned_atoms: RwLock<HashMap<api::AtomId, WeakAtomHand>>,
	pub common_exprs: ExprStore,
	pub root: OnceCell<Weak<Root>>,
}
#[derive(Clone)]
pub struct Ctx(Rc<CtxData>);
impl ops::Deref for Ctx {
	type Target = CtxData;
	fn deref(&self) -> &Self::Target { &self.0 }
}
impl Ctx {
	pub fn new(spawn: Spawner) -> Self {
		Self(Rc::new(CtxData {
			spawn,
			i: Rc::default(),
			systems: RwLock::default(),
			system_id: RefCell::new(NonZero::new(1).unwrap()),
			owned_atoms: RwLock::default(),
			common_exprs: ExprStore::default(),
			root: OnceCell::default(),
		}))
	}
	pub(crate) async fn system_inst(&self, id: api::SysId) -> Option<System> {
		self.systems.read().await.get(&id).and_then(WeakSystem::upgrade)
	}
	pub(crate) fn next_sys_id(&self) -> api::SysId {
		let mut g = self.system_id.borrow_mut();
		*g = g.checked_add(1).unwrap_or(NonZeroU16::new(1).unwrap());
		SysId(*g)
	}
	pub async fn set_root(&self, root: Weak<Root>) {
		assert!(self.root.get().is_none(), "Root already assigned");
		self.root.get_or_init(async { root }).await;
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
