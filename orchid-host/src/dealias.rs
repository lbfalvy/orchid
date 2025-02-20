use std::rc::Rc;

use futures::FutureExt;
use hashbrown::{HashMap, HashSet};
use itertools::{Either, Itertools};
use orchid_base::error::{OrcErr, Reporter, mk_err};
use orchid_base::format::{FmtCtxImpl, Format, take_first};
use orchid_base::interner::{Interner, Tok};
use orchid_base::location::Pos;
use orchid_base::name::{NameLike, Sym, VName};

use crate::macros::{MacTok, MacTree};
use crate::tree::{ItemKind, MemberKind, Module, RuleKind, WalkErrorKind};

/// Errors produced by absolute_path
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum AbsPathError {
	/// `super` of root requested, for example, `app::main` referenced
	/// `super::super::super::std`
	TooManySupers,
	/// root selected, for example, `app::main` referenced exactly `super::super`.
	/// The empty path also triggers this.
	RootPath,
}
impl AbsPathError {
	pub async fn err_obj(self, i: &Interner, pos: Pos, path: &str) -> OrcErr {
		let (descr, msg) = match self {
			AbsPathError::RootPath => (
				i.i("Path ends on root module").await,
				format!(
					"{path} is equal to the empty path. You cannot directly reference the root. \
					Use one fewer 'super::' or add more segments to make it valid."
				),
			),
			AbsPathError::TooManySupers => (
				i.i("Too many 'super::' steps in path").await,
				format!("{path} is leading outside the root."),
			),
		};
		mk_err(descr, msg, [pos.into()])
	}
}

/// Turn a relative (import) path into an absolute path.
/// If the import path is empty, the return value is also empty.
///
/// # Errors
///
/// if the relative path contains as many or more `super` segments than the
/// length of the absolute path.
pub fn absolute_path(
	mut cwd: &[Tok<String>],
	mut rel: &[Tok<String>],
) -> Result<VName, AbsPathError> {
	let mut relative = false;
	if rel.first().map(|t| t.as_str()) == Some("self") {
		relative = true;
		rel = rel.split_first().expect("checked above").1;
	} else {
		while rel.first().map(|t| t.as_str()) == Some("super") {
			match cwd.split_last() {
				Some((_, torso)) => cwd = torso,
				None => return Err(AbsPathError::TooManySupers),
			};
			rel = rel.split_first().expect("checked above").1;
			relative = true;
		}
	}
	match relative {
		true => VName::new(cwd.iter().chain(rel).cloned()),
		false => VName::new(rel.to_vec()),
	}
	.map_err(|_| AbsPathError::RootPath)
}

pub async fn resolv_glob(
	cwd: &[Tok<String>],
	root: &Module,
	abs_path: &[Tok<String>],
	pos: Pos,
	i: &Interner,
	r: &impl Reporter,
) -> Vec<Tok<String>> {
	let coprefix_len = cwd.iter().zip(abs_path).take_while(|(a, b)| a == b).count();
	let (co_prefix, diff_path) = abs_path.split_at(coprefix_len);
	let co_parent = root.walk(false, co_prefix.iter().cloned()).await.expect("Invalid step in cwd");
	let target_module = match co_parent.walk(true, diff_path.iter().cloned()).await {
		Ok(t) => t,
		Err(e) => {
			let path = abs_path[..=coprefix_len + e.pos].iter().join("::");
			let (tk, msg) = match e.kind {
				WalkErrorKind::Constant =>
					(i.i("Invalid import path").await, format!("{path} is a constant")),
				WalkErrorKind::Missing => (i.i("Invalid import path").await, format!("{path} not found")),
				WalkErrorKind::Private => (i.i("Import inaccessible").await, format!("{path} is private")),
			};
			r.report(mk_err(tk, msg, [pos.into()]));
			return vec![];
		},
	};
	target_module.exports.clone()
}

/// Read import statements and convert them into aliases, rasising any import
/// errors in the process
pub async fn imports_to_aliases(
	module: &Module,
	cwd: &mut Vec<Tok<String>>,
	root: &Module,
	alias_map: &mut HashMap<Sym, Sym>,
	alias_rev_map: &mut HashMap<Sym, HashSet<Sym>>,
	i: &Interner,
	rep: &impl Reporter,
) {
	let mut import_locs = HashMap::<Sym, Vec<Pos>>::new();
	for item in &module.items {
		match &item.kind {
			ItemKind::Import(imp) => match absolute_path(cwd, &imp.path) {
				Err(e) => rep.report(e.err_obj(i, item.pos.clone(), &imp.path.iter().join("::")).await),
				Ok(abs_path) => {
					let names = match imp.name.as_ref() {
						Some(n) => Either::Right([n.clone()].into_iter()),
						None => Either::Left(
							resolv_glob(cwd, root, &abs_path, item.pos.clone(), i, rep).await.into_iter(),
						),
					};
					for name in names {
						let mut tgt = abs_path.clone().suffix([name.clone()]).to_sym(i).await;
						let src = Sym::new(cwd.iter().cloned().chain([name]), i).await.unwrap();
						import_locs.entry(src.clone()).or_insert(vec![]).push(item.pos.clone());
						if let Some(tgt2) = alias_map.get(&tgt) {
							tgt = tgt2.clone();
						}
						if src == tgt {
							rep.report(mk_err(
								i.i("Circular references").await,
								format!("{src} circularly refers to itself"),
								[item.pos.clone().into()],
							));
							continue;
						}
						if let Some(fst_val) = alias_map.get(&src) {
							let locations = (import_locs.get(&src))
								.expect("The same name could only have appeared in the same module");
							rep.report(mk_err(
								i.i("Conflicting imports").await,
								if fst_val == &src {
									format!("{src} is imported multiple times")
								} else {
									format!("{} could refer to both {fst_val} and {src}", src.last())
								},
								locations.iter().map(|p| p.clone().into()).collect_vec(),
							))
						}
						let mut srcv = vec![src.clone()];
						if let Some(src_extra) = alias_rev_map.remove(&src) {
							srcv.extend(src_extra);
						}
						for src in srcv {
							alias_map.insert(src.clone(), tgt.clone());
							alias_rev_map.entry(tgt.clone()).or_insert(HashSet::new()).insert(src);
						}
					}
				},
			},
			ItemKind::Member(mem) => match mem.kind().await {
				MemberKind::Const(_) => (),
				MemberKind::Mod(m) => {
					cwd.push(mem.name());
					imports_to_aliases(m, cwd, root, alias_map, alias_rev_map, i, rep).boxed_local().await;
					cwd.pop();
				},
			},
			ItemKind::Export(_) | ItemKind::Macro(..) => (),
		}
	}
}

pub async fn dealias(module: &mut Module, alias_map: &HashMap<Sym, Sym>, i: &Interner) {
	for item in &mut module.items {
		match &mut item.kind {
			ItemKind::Export(_) | ItemKind::Import(_) => (),
			ItemKind::Member(mem) => match mem.kind_mut().await {
				MemberKind::Const(c) => {
					let Some(source) = c.source() else { continue };
					let Some(new_source) = dealias_mactreev(source, alias_map, i).await else { continue };
					c.set_source(new_source);
				},
				MemberKind::Mod(m) => dealias(m, alias_map, i).boxed_local().await,
			},
			ItemKind::Macro(_, rules) =>
				for rule in rules.iter_mut() {
					let RuleKind::Native(c) = &mut rule.kind else { continue };
					let Some(source) = c.source() else { continue };
					let Some(new_source) = dealias_mactreev(source, alias_map, i).await else { continue };
					c.set_source(new_source);
				},
		}
	}
}

async fn dealias_mactree(
	mtree: &MacTree,
	aliases: &HashMap<Sym, Sym>,
	i: &Interner,
) -> Option<MacTree> {
	let new_tok = match &*mtree.tok {
		MacTok::Atom(_) | MacTok::Ph(_) => return None,
		tok @ (MacTok::Done(_) | MacTok::Ref(_) | MacTok::Slot(_)) => panic!(
			"{} should not appear in retained pre-macro source",
			take_first(&tok.print(&FmtCtxImpl { i }).await, true)
		),
		MacTok::Name(n) => MacTok::Name(aliases.get(n).unwrap_or(n).clone()),
		MacTok::Lambda(arg, body) => {
			match (dealias_mactreev(arg, aliases, i).await, dealias_mactreev(body, aliases, i).await) {
				(None, None) => return None,
				(Some(arg), None) => MacTok::Lambda(arg, body.clone()),
				(None, Some(body)) => MacTok::Lambda(arg.clone(), body),
				(Some(arg), Some(body)) => MacTok::Lambda(arg, body),
			}
		},
		MacTok::S(p, b) => MacTok::S(*p, dealias_mactreev(b, aliases, i).await?),
	};
	Some(MacTree { pos: mtree.pos.clone(), tok: Rc::new(new_tok) })
}

async fn dealias_mactreev(
	mtreev: &[MacTree],
	aliases: &HashMap<Sym, Sym>,
	i: &Interner,
) -> Option<Vec<MacTree>> {
	let mut results = Vec::with_capacity(mtreev.len());
	let mut any_some = false;
	for item in mtreev {
		let out = dealias_mactree(item, aliases, i).boxed_local().await;
		any_some |= out.is_some();
		results.push(out.unwrap_or(item.clone()));
	}
	any_some.then_some(results)
}
