use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use hashbrown::HashMap;
use hashbrown::hash_map::Entry;

use crate::api;
use crate::expr::Expr;

#[derive(Default)]
pub struct ExprStoreData {
	exprs: RefCell<HashMap<api::ExprTicket, (u32, Expr)>>,
	parent: Option<ExprStore>,
}
#[derive(Clone, Default)]
pub struct ExprStore(Rc<ExprStoreData>);
impl ExprStore {
	pub fn derive(&self) -> Self {
		Self(Rc::new(ExprStoreData { exprs: RefCell::default(), parent: Some(self.clone()) }))
	}
	pub fn give_expr(&self, expr: Expr) {
		match self.0.exprs.borrow_mut().entry(expr.id()) {
			Entry::Occupied(mut oe) => oe.get_mut().0 += 1,
			Entry::Vacant(v) => {
				v.insert((1, expr));
			},
		}
	}
	pub fn take_expr(&self, ticket: api::ExprTicket) {
		(self.0.exprs.borrow_mut().entry(ticket))
			.and_replace_entry_with(|_, (rc, rt)| (1 < rc).then_some((rc - 1, rt)));
	}
	pub fn get_expr(&self, ticket: api::ExprTicket) -> Option<Expr> {
		(self.0.exprs.borrow().get(&ticket).map(|(_, expr)| expr.clone()))
			.or_else(|| self.0.parent.as_ref()?.get_expr(ticket))
	}
}
impl fmt::Display for ExprStore {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let r = self.0.exprs.borrow();
		let rc: u32 = r.values().map(|v| v.0).sum();
		write!(f, "Store holding {rc} refs to {} exprs", r.len())
	}
}
