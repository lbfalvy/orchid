use std::cell::RefCell;
use std::rc::{Rc, Weak};

use async_once_cell::OnceCell;
use hashbrown::HashMap;
use orchid_base::interner::Tok;
use orchid_base::name::Sym;

use crate::expr::Expr;
use crate::parsed::{LazyMemberHandle, ParsedMemberKind, ParsedModule};

pub struct Tree(Rc<Module>);

pub struct WeakTree(Weak<Module>);

pub struct Module {
	pub members: HashMap<Tok<String>, Rc<Member>>,
}
impl Module {
	async fn from_parsed(parsed: &ParsedModule, root: &ParsedModule) -> Self {
		let imports = 
	}
}

pub struct Member {
	pub public: bool,
	pub root: WeakTree,
	pub canonical_path: Sym,
	pub lazy: RefCell<Option<(LazyMemberHandle, Rc<ParsedModule>)>>,
	pub kind: OnceCell<MemberKind>,
}
impl Member {
	pub async fn kind_mut(&mut self, consts: &mut HashMap<Sym, Expr>) -> &mut MemberKind {
		self.kind(consts).await;
		self.kind.get_mut().expect("Thhe above line should have initialized it")
	}
	pub async fn kind(&self, consts: &mut HashMap<Sym, Expr>) -> &MemberKind {
		(self.kind.get_or_init(async {
			let (handle, root) =
				self.lazy.borrow_mut().take().expect("If kind is uninit, lazy must be Some");
			let parsed = handle.run(consts).await;
			MemberKind::from_parsed(&parsed, &root).await
		}))
		.await
	}
}

pub enum MemberKind {
	Const,
	Module(Module),
}
impl MemberKind {
	async fn from_parsed(parsed: &ParsedMemberKind, root: &ParsedModule) -> Self {
		match parsed {
			ParsedMemberKind::Const => MemberKind::Const,
			ParsedMemberKind::Mod(m) => MemberKind::Module(Module::from_parsed(m, root).await),
		}
	}
}
