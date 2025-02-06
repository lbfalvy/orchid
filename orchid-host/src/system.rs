use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::rc::{Rc, Weak};

use async_stream::stream;
use derive_destructure::destructure;
use futures::StreamExt;
use futures::future::join_all;
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_base::async_once_cell::OnceCell;
use orchid_base::char_filter::char_filter_match;
use orchid_base::clone;
use orchid_base::error::{OrcErrv, OrcRes};
use orchid_base::format::{FmtCtx, FmtUnit, Format};
use orchid_base::interner::Tok;
use orchid_base::parse::Comment;
use orchid_base::reqnot::{ReqNot, Requester};
use orchid_base::tree::ttv_from_api;
use ordered_float::NotNan;
use substack::{Stackframe, Substack};

use crate::api;
use crate::ctx::Ctx;
use crate::extension::{Extension, WeakExtension};
use crate::tree::{Member, ParsTokTree};

#[derive(destructure)]
struct SystemInstData {
	ctx: Ctx,
	ext: Extension,
	decl_id: api::SysDeclId,
	lex_filter: api::CharFilter,
	id: api::SysId,
	const_root: OnceCell<Vec<Member>>,
	line_types: Vec<Tok<String>>,
}
impl Drop for SystemInstData {
	fn drop(&mut self) { self.ext.system_drop(self.id); }
}
impl fmt::Debug for SystemInstData {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SystemInstData")
			.field("decl_id", &self.decl_id)
			.field("lex_filter", &self.lex_filter)
			.field("id", &self.id)
			.field("const_root", &self.const_root)
			.field("line_types", &self.line_types)
			.finish_non_exhaustive()
	}
}

#[derive(Clone, Debug)]
pub struct System(Rc<SystemInstData>);
impl System {
	pub fn id(&self) -> api::SysId { self.0.id }
	pub fn ext(&self) -> &Extension { &self.0.ext }
	pub fn ctx(&self) -> &Ctx { &self.0.ctx }
	pub(crate) fn reqnot(&self) -> &ReqNot<api::HostMsgSet> { self.0.ext.reqnot() }
	pub async fn get_tree(&self, id: api::TreeId) -> api::MemberKind {
		self.reqnot().request(api::GetMember(self.0.id, id)).await
	}
	pub fn has_lexer(&self) -> bool { !self.0.lex_filter.0.is_empty() }
	pub fn can_lex(&self, c: char) -> bool { char_filter_match(&self.0.lex_filter, c) }
	/// Have this system lex a part of the source. It is assumed that
	/// [Self::can_lex] was called and returned true.
	pub async fn lex<F: Future<Output = Option<api::SubLexed>>>(
		&self,
		source: Tok<String>,
		pos: u32,
		r: impl FnMut(u32) -> F,
	) -> api::OrcResult<Option<api::LexedExpr>> {
		self.0.ext.lex_req(source, pos, self.id(), r).await
	}
	pub fn can_parse(&self, ltyp: Tok<String>) -> bool { self.0.line_types.contains(&ltyp) }
	pub fn line_types(&self) -> impl Iterator<Item = &Tok<String>> + '_ { self.0.line_types.iter() }
	pub async fn parse(
		&self,
		line: Vec<ParsTokTree>,
		exported: bool,
		comments: Vec<Comment>,
	) -> OrcRes<Vec<ParsTokTree>> {
		let line =
			join_all(line.iter().map(|t| async { t.to_api(&mut |n, _| match *n {}).await })).await;
		let comments = comments.iter().map(Comment::to_api).collect_vec();
		match self.reqnot().request(api::ParseLine { exported, sys: self.id(), comments, line }).await {
			Ok(parsed) => Ok(ttv_from_api(parsed, &mut self.ctx().clone(), &self.ctx().i).await),
			Err(e) => Err(OrcErrv::from_api(&e, &self.ctx().i).await),
		}
	}
	pub async fn request(&self, req: Vec<u8>) -> Vec<u8> {
		self.reqnot().request(api::SysFwded(self.id(), req)).await
	}
	pub(crate) fn drop_atom(&self, drop: api::AtomId) {
		let this = self.0.clone();
		(self.0.ctx.spawn)(Box::pin(async move {
			this.ctx.owned_atoms.write().await.remove(&drop);
		}))
	}
	pub fn downgrade(&self) -> WeakSystem { WeakSystem(Rc::downgrade(&self.0)) }
}
impl Format for System {
	async fn print<'a>(&'a self, _c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		let ctor = (self.0.ext.system_ctors().find(|c| c.id() == self.0.decl_id))
			.expect("System instance with no associated constructor");
		format!("System({} @ {} #{})", ctor.name(), ctor.priority(), self.0.id.0).into()
	}
}

pub struct WeakSystem(Weak<SystemInstData>);
impl WeakSystem {
	pub fn upgrade(&self) -> Option<System> { self.0.upgrade().map(System) }
}

pub struct SystemCtor {
	pub(crate) decl: api::SystemDecl,
	pub(crate) ext: WeakExtension,
}
impl SystemCtor {
	pub fn name(&self) -> &str { &self.decl.name }
	pub fn priority(&self) -> NotNan<f64> { self.decl.priority }
	pub fn depends(&self) -> impl ExactSizeIterator<Item = &str> {
		self.decl.depends.iter().map(|s| &**s)
	}
	pub fn id(&self) -> api::SysDeclId { self.decl.id }
	pub async fn run<'a>(&self, depends: impl IntoIterator<Item = &'a System>) -> System {
		let depends = depends.into_iter().map(|si| si.id()).collect_vec();
		debug_assert_eq!(depends.len(), self.decl.depends.len(), "Wrong number of deps provided");
		let ext = self.ext.upgrade().expect("SystemCtor should be freed before Extension");
		let id = ext.ctx().next_sys_id();
		let sys_inst = ext.reqnot().request(api::NewSystem { depends, id, system: self.decl.id }).await;
		let data = System(Rc::new(SystemInstData {
			decl_id: self.decl.id,
			ext: ext.clone(),
			ctx: ext.ctx().clone(),
			lex_filter: sys_inst.lex_filter,
			const_root: OnceCell::new(),
			line_types: join_all(sys_inst.line_types.iter().map(|m| Tok::from_api(*m, &ext.ctx().i)))
				.await,
			id,
		}));
		(data.0.const_root.get_or_init(
			clone!(data, ext; stream! {
				for (k, v) in sys_inst.const_root {
					yield Member::from_api(
						api::Member { name: k, kind: v },
						&mut vec![Tok::from_api(k, &ext.ctx().i).await],
						&data,
					).await
				}
			})
			.collect(),
		))
		.await;
		ext.ctx().systems.write().await.insert(id, data.downgrade());
		data
	}
}

#[derive(Debug, Clone)]
pub enum SysResolvErr {
	Loop(Vec<String>),
	Missing(String),
}

pub async fn init_systems(
	tgts: &[String],
	exts: &[Extension],
) -> Result<Vec<System>, SysResolvErr> {
	let mut to_load = HashMap::<&str, &SystemCtor>::new();
	let mut to_find = tgts.iter().map(|s| s.as_str()).collect::<VecDeque<&str>>();
	while let Some(target) = to_find.pop_front() {
		if to_load.contains_key(target) {
			continue;
		}
		let ctor = (exts.iter())
			.flat_map(|e| e.system_ctors().filter(|c| c.name() == target))
			.max_by_key(|c| c.priority())
			.ok_or_else(|| SysResolvErr::Missing(target.to_string()))?;
		to_load.insert(target, ctor);
		to_find.extend(ctor.depends());
	}
	let mut to_load_ordered = Vec::new();
	fn walk_deps<'a>(
		graph: &mut HashMap<&str, &'a SystemCtor>,
		list: &mut Vec<&'a SystemCtor>,
		chain: Stackframe<&str>,
	) -> Result<(), SysResolvErr> {
		if let Some(ctor) = graph.remove(chain.item) {
			// if the above is none, the system is already queued. Missing systems are
			// detected above
			for dep in ctor.depends() {
				if Substack::Frame(chain).iter().any(|c| *c == dep) {
					let mut circle = vec![dep.to_string()];
					circle.extend(Substack::Frame(chain).iter().map(|s| s.to_string()));
					return Err(SysResolvErr::Loop(circle));
				}
				walk_deps(graph, list, Substack::Frame(chain).new_frame(dep))?
			}
			list.push(ctor);
		}
		Ok(())
	}
	for tgt in tgts {
		walk_deps(&mut to_load, &mut to_load_ordered, Substack::Bottom.new_frame(tgt))?;
	}
	let mut systems = HashMap::<&str, System>::new();
	for ctor in to_load_ordered.iter() {
		let sys = ctor.run(ctor.depends().map(|n| &systems[n])).await;
		systems.insert(ctor.name(), sys);
	}
	Ok(systems.into_values().collect_vec())
}
