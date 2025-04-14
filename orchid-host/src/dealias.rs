use std::rc::Rc;

use futures::FutureExt;
use hashbrown::{HashMap, HashSet};
use itertools::{Either, Itertools};
use orchid_base::error::{OrcErr, Reporter, mk_err};
use orchid_base::format::{FmtCtxImpl, Format, take_first};
use orchid_base::interner::{Interner, Tok};
use orchid_base::location::Pos;
use orchid_base::name::{NameLike, Sym, VName};
use substack::Substack;

use crate::expr::Expr;
use crate::parsed::{ItemKind, ParsedMemberKind, ParsedModule, WalkErrorKind};

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

pub struct DealiasCtx<'a> {
	pub i: &'a Interner,
	pub rep: &'a Reporter,
	pub consts: &'a mut HashMap<Sym, Expr>,
}

pub async fn resolv_glob(
	cwd: &[Tok<String>],
	root: &ParsedModule,
	abs_path: &[Tok<String>],
	pos: Pos,
	ctx: &mut DealiasCtx<'_>,
) -> Vec<Tok<String>> {
	let coprefix_len = cwd.iter().zip(abs_path).take_while(|(a, b)| a == b).count();
	let (co_prefix, diff_path) = abs_path.split_at(coprefix_len);
	let co_parent =
		root.walk(false, co_prefix.iter().cloned(), ctx.consts).await.expect("Invalid step in cwd");
	let target_module = match co_parent.walk(true, diff_path.iter().cloned(), ctx.consts).await {
		Ok(t) => t,
		Err(e) => {
			let path = abs_path[..=coprefix_len + e.pos].iter().join("::");
			let (tk, msg) = match e.kind {
				WalkErrorKind::Constant =>
					(ctx.i.i("Invalid import path").await, format!("{path} is a constant")),
				WalkErrorKind::Missing =>
					(ctx.i.i("Invalid import path").await, format!("{path} not found")),
				WalkErrorKind::Private =>
					(ctx.i.i("Import inaccessible").await, format!("{path} is private")),
			};
			(&ctx.rep).report(mk_err(tk, msg, [pos.into()]));
			return vec![];
		},
	};
	target_module.exports.clone()
}

/// Read import statements and convert them into aliases, rasising any import
/// errors in the process
pub async fn imports_to_aliases(
	module: &ParsedModule,
	cwd: &mut Vec<Tok<String>>,
	root: &ParsedModule,
	alias_map: &mut HashMap<Sym, Sym>,
	alias_rev_map: &mut HashMap<Sym, HashSet<Sym>>,
	ctx: &mut DealiasCtx<'_>,
) {
	let mut import_locs = HashMap::<Sym, Vec<Pos>>::new();
	for item in &module.items {
		match &item.kind {
			ItemKind::Import(imp) => match absolute_path(cwd, &imp.path) {
				Err(e) =>
					ctx.rep.report(e.err_obj(ctx.i, item.pos.clone(), &imp.path.iter().join("::")).await),
				Ok(abs_path) => {
					let names = match imp.name.as_ref() {
						Some(n) => Either::Right([n.clone()].into_iter()),
						None => Either::Left(
							resolv_glob(cwd, root, &abs_path, item.pos.clone(), ctx).await.into_iter(),
						),
					};
					for name in names {
						let mut tgt = abs_path.clone().suffix([name.clone()]).to_sym(ctx.i).await;
						let src = Sym::new(cwd.iter().cloned().chain([name]), ctx.i).await.unwrap();
						import_locs.entry(src.clone()).or_insert(vec![]).push(item.pos.clone());
						if let Some(tgt2) = alias_map.get(&tgt) {
							tgt = tgt2.clone();
						}
						if src == tgt {
							ctx.rep.report(mk_err(
								ctx.i.i("Circular references").await,
								format!("{src} circularly refers to itself"),
								[item.pos.clone().into()],
							));
							continue;
						}
						if let Some(fst_val) = alias_map.get(&src) {
							let locations = (import_locs.get(&src))
								.expect("The same name could only have appeared in the same module");
							ctx.rep.report(mk_err(
								ctx.i.i("Conflicting imports").await,
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
			ItemKind::Member(mem) => match mem.kind(ctx.consts).await {
				ParsedMemberKind::Const => (),
				ParsedMemberKind::Mod(m) => {
					cwd.push(mem.name());
					imports_to_aliases(m, cwd, root, alias_map, alias_rev_map, ctx).boxed_local().await;
					cwd.pop();
				},
			},
			ItemKind::Export(_) => (),
		}
	}
}

pub async fn dealias(
	path: Substack<'_, Tok<String>>,
	module: &mut ParsedModule,
	alias_map: &HashMap<Sym, Sym>,
	ctx: &mut DealiasCtx<'_>,
) {
	for item in &mut module.items {
		match &mut item.kind {
			ItemKind::Export(_) | ItemKind::Import(_) => (),
			ItemKind::Member(mem) => {
				let path = path.push(mem.name());
				match mem.kind_mut(ctx.consts).await {
					ParsedMemberKind::Const => (),
					ParsedMemberKind::Mod(m) => dealias(path, m, alias_map, ctx).boxed_local().await,
				}
			},
		}
	}
}
