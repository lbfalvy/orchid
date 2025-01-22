use std::rc::Rc;

use ahash::HashMap;
use futures::future::{LocalBoxFuture, join_all};
use itertools::Itertools;
use never::Never;
use orchid_base::error::OrcRes;
use orchid_base::interner::Tok;
use orchid_base::macros::{MTree, mtreev_from_api, mtreev_to_api};
use orchid_base::reqnot::Requester;
use trait_set::trait_set;

use crate::api;
use crate::atom::AtomFactory;
use crate::lexer::err_cascade;
use crate::system::SysCtx;
use crate::tree::TreeIntoApiCtx;

pub trait Macro {
	fn pattern() -> MTree<'static, Never>;
	fn apply(binds: HashMap<Tok<String>, MTree<'_, Never>>) -> MTree<'_, AtomFactory>;
}

pub trait DynMacro {
	fn pattern(&self) -> MTree<'static, Never>;
	fn apply<'a>(&self, binds: HashMap<Tok<String>, MTree<'a, Never>>) -> MTree<'a, AtomFactory>;
}

impl<T: Macro> DynMacro for T {
	fn pattern(&self) -> MTree<'static, Never> { Self::pattern() }
	fn apply<'a>(&self, binds: HashMap<Tok<String>, MTree<'a, Never>>) -> MTree<'a, AtomFactory> {
		Self::apply(binds)
	}
}

pub struct RuleCtx<'a> {
	pub(crate) args: HashMap<Tok<String>, Vec<MTree<'a, Never>>>,
	pub(crate) run_id: api::ParsId,
	pub(crate) sys: SysCtx,
}
impl<'a> RuleCtx<'a> {
	pub async fn recurse(&mut self, tree: &[MTree<'a, Never>]) -> OrcRes<Vec<MTree<'a, Never>>> {
		let req = api::RunMacros {
			run_id: self.run_id,
			query: mtreev_to_api(tree, &mut |b| match *b {}).await,
		};
		let Some(treev) = self.sys.reqnot.request(req).await else {
			return Err(err_cascade(&self.sys.i).await.into());
		};
		static ATOM_MSG: &str = "Returned atom from Rule recursion";
		Ok(mtreev_from_api(&treev, &mut |_| panic!("{ATOM_MSG}"), &self.sys.i).await)
	}
	pub fn getv(&mut self, key: &Tok<String>) -> Vec<MTree<'a, Never>> {
		self.args.remove(key).expect("Key not found")
	}
	pub fn gets(&mut self, key: &Tok<String>) -> MTree<'a, Never> {
		let v = self.getv(key);
		assert!(v.len() == 1, "Not a scalar");
		v.into_iter().next().unwrap()
	}
	pub fn unused_arg<'b>(&mut self, keys: impl IntoIterator<Item = &'b Tok<String>>) {
		keys.into_iter().for_each(|k| {
			self.getv(k);
		});
	}
}

trait_set! {
	pub trait RuleCB = for<'a> Fn(RuleCtx<'a>) -> LocalBoxFuture<'a, OrcRes<Vec<MTree<'a, AtomFactory>>>>;
}

pub struct Rule {
	pub(crate) comments: Vec<String>,
	pub(crate) pattern: Vec<MTree<'static, Never>>,
	pub(crate) apply: Rc<dyn RuleCB>,
}
impl Rule {
	pub(crate) async fn into_api(self, ctx: &mut impl TreeIntoApiCtx) -> api::MacroRule {
		api::MacroRule {
			comments: join_all(self.comments.iter().map(|c| async {
				api::Comment { text: ctx.sys().i.i(c).await.to_api(), location: api::Location::Inherit }
			}))
			.await,
			location: api::Location::Inherit,
			pattern: mtreev_to_api(&self.pattern, &mut |b| match *b {}).await,
			id: ctx.with_rule(Rc::new(self)),
		}
	}
}

pub fn rule_cmt<'a>(
	cmt: impl IntoIterator<Item = &'a str>,
	pattern: Vec<MTree<'static, Never>>,
	apply: impl RuleCB + 'static,
) -> Rule {
	let comments = cmt.into_iter().map(|s| s.to_string()).collect_vec();
	Rule { comments, pattern, apply: Rc::new(apply) }
}

pub fn rule(pattern: Vec<MTree<'static, Never>>, apply: impl RuleCB + 'static) -> Rule {
	rule_cmt([], pattern, apply)
}
