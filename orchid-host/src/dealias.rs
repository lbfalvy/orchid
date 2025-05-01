use std::collections::VecDeque;

use futures::FutureExt;
use hashbrown::{HashMap, HashSet};
use itertools::{Either, Itertools};
use orchid_base::error::{OrcErr, Reporter, mk_err};
use orchid_base::interner::{Interner, Tok};
use orchid_base::location::Pos;
use orchid_base::name::{NameLike, Sym, VName};
use substack::Substack;

use crate::expr::Expr;
use crate::parsed::{ItemKind, ParsedMemberKind, ParsedModule};

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
}

pub async fn resolv_glob<Mod: Tree>(
	cwd: &[Tok<String>],
	root: &Mod,
	abs_path: &[Tok<String>],
	pos: Pos,
	i: &Interner,
	rep: &Reporter,
	walk_cx: &mut Mod::Ctx,
) -> Vec<Tok<String>> {
	let coprefix_len = cwd.iter().zip(abs_path).take_while(|(a, b)| a == b).count();
	let (co_prefix, diff_path) = abs_path.split_at(coprefix_len);
	let co_parent =
		root.walk(true, co_prefix.iter().cloned(), walk_cx).await.expect("Invalid step in cwd");
	let target_module = match co_parent.walk(false, diff_path.iter().cloned(), walk_cx).await {
		Ok(t) => t,
		Err(e) => {
			let path = abs_path[..=coprefix_len + e.pos].iter().join("::");
			let (tk, msg) = match e.kind {
				ChildErrorKind::Constant =>
					(i.i("Invalid import path").await, format!("{path} is a const")),
				ChildErrorKind::Missing => (i.i("Invalid import path").await, format!("{path} not found")),
				ChildErrorKind::Private => (i.i("Import inaccessible").await, format!("{path} is private")),
			};
			rep.report(mk_err(tk, msg, [pos.into()]));
			return vec![];
		},
	};
	target_module.children(false)
}

pub enum ChildResult<'a, T: Tree + ?Sized> {
	Value(&'a T),
	Err(ChildErrorKind),
	Alias(&'a [Tok<String>]),
}
pub trait Tree {
	type Ctx;
	fn children(&self, public_only: bool) -> HashSet<Tok<String>>;
	fn child(&self, key: Tok<String>, public_only: bool) -> ChildResult<'_, Self>;
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ChildErrorKind {
	Missing,
	Private,
	Constant,
}
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ChildError {
	pub at_path: Vec<Tok<String>>,
	pub kind: ChildErrorKind,
}

// Problem: walk should take into account aliases and visibility
//
// help: since every alias is also its own import, visibility only has to be
// checked on the top level
//
// idea: do a simple stack machine like below with no visibility for aliases and
// call it from an access-checking implementation for just the top level
//
// caveat: we need to check EVERY IMPORT to ensure that all
// errors are raised

fn walk_no_access_chk<T: Tree>(
	root: &T,
	path: impl IntoIterator<Item = Tok<String>>,
) -> Result<&T, ChildError> {
	let mut cur = root;
	let mut cur_path = Vec::new();
	let mut path = VecDeque::from(path);
	while let Some(step) = path.pop_front() {
		match cur.child(step, false) {
			ChildResult::Alias(target) => {
				path.reserve(target.len());
				target.iter().rev().for_each(|tok| path.push_front(tok.clone()));
				cur = root;
				cur_path = Vec::new();
			},
			ChildResult::Err(e) => return Err(ChildError { pos: (), kind: () }),
		}
	}
}
