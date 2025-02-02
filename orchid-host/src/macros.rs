use std::rc::Rc;

use futures::FutureExt;
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use orchid_base::clone;
use orchid_base::macros::{MTok, MTree, mtreev_from_api, mtreev_to_api};
use orchid_base::name::Sym;
use trait_set::trait_set;

use crate::api;
use crate::atom::AtomHand;
use crate::ctx::Ctx;
use crate::rule::state::MatchState;
use crate::tree::Code;

pub type MacTok = MTok<'static, AtomHand>;
pub type MacTree = MTree<'static, AtomHand>;

trait_set! {
	trait MacroCB = Fn(Vec<MacTree>) -> Option<Vec<MacTree>>;
}

type Slots = HashMap<api::MacroTreeId, Rc<MacTok>>;

pub async fn macro_treev_to_api(mtree: Vec<MacTree>, slots: &mut Slots) -> Vec<api::MacroTree> {
	mtreev_to_api(&mtree, &mut |a: &AtomHand| {
		let id = api::MacroTreeId((slots.len() as u64 + 1).try_into().unwrap());
		slots.insert(id, Rc::new(MacTok::Atom(a.clone())));
		async move { api::MacroToken::Slot(id) }.boxed_local()
	})
	.await
}

pub async fn macro_treev_from_api(api: Vec<api::MacroTree>, ctx: Ctx) -> Vec<MacTree> {
	mtreev_from_api(&api, &ctx.clone().i, &mut move |atom| {
		clone!(ctx);
		Box::pin(async move { MacTok::Atom(AtomHand::new(atom.clone(), &ctx).await) })
	})
	.await
}

pub fn deslot_macro(tree: &[MacTree], slots: &mut Slots) -> Option<Vec<MacTree>> {
	return work(slots, tree);
	fn work(
		slots: &mut HashMap<api::MacroTreeId, Rc<MacTok>>,
		tree: &[MacTree],
	) -> Option<Vec<MacTree>> {
		let items = (tree.iter())
			.map(|t| {
				Some(MacTree {
					tok: match &*t.tok {
						MacTok::Atom(_) | MacTok::Name(_) | MacTok::Ph(_) => return None,
						MacTok::Ref(_) => panic!("Ref is an extension-local optimization"),
						MacTok::Done(_) => panic!("Created and removed by matcher"),
						MacTok::Slot(slot) => slots.get(&slot.id()).expect("Slot not found").clone(),
						MacTok::S(paren, b) => Rc::new(MacTok::S(*paren, work(slots, b)?)),
						MacTok::Lambda(a, b) => Rc::new(match (work(slots, a), work(slots, b)) {
							(None, None) => return None,
							(Some(a), None) => MacTok::Lambda(a, b.clone()),
							(None, Some(b)) => MacTok::Lambda(a.clone(), b),
							(Some(a), Some(b)) => MacTok::Lambda(a, b),
						}),
					},
					pos: t.pos.clone(),
				})
			})
			.collect_vec();
		let any_changed = items.iter().any(Option::is_some);
		any_changed.then(|| {
			(items.into_iter().enumerate())
				.map(|(i, opt)| opt.unwrap_or_else(|| tree[i].clone()))
				.collect_vec()
		})
	}
}

pub struct Macro<Matcher> {
	deps: HashSet<Sym>,
	cases: Vec<(Matcher, Code)>,
}

fn fill_lexicon(tgt: &MacTree, lexicon: &mut HashSet<Sym>) {
	match &*tgt.tok {
		MTok::Name(n) => {
			lexicon.insert(n.clone());
		},
		MTok::Lambda(arg, body) => {
			arg.iter().for_each(|t| fill_lexicon(t, lexicon));
			body.iter().for_each(|t| fill_lexicon(t, lexicon))
		},
		MTok::S(_, body) => body.iter().for_each(|t| fill_lexicon(t, lexicon)),
		_ => (),
	}
}

fn run_body(body: &Code, mut state: MatchState<'_>) -> Vec<MacTree> {
	let inject: Vec<MacTree> = todo!("Call the interpreter with bindings");
	inject
		.into_iter()
		.map(|MTree { pos, tok }| MTree { pos, tok: Rc::new(MTok::Done(tok)) })
		.collect_vec()
}
