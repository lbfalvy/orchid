use std::collections::VecDeque;
use std::num::NonZero;
use std::ops::Deref;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::{fmt, io, thread};

use derive_destructure::destructure;
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use itertools::Itertools;
use lazy_static::lazy_static;
use orchid_api_traits::Request;
use orchid_base::builtin::{ExtFactory, ExtPort};
use orchid_base::char_filter::char_filter_match;
use orchid_base::clone;
use orchid_base::error::{OrcErrv, OrcRes};
use orchid_base::interner::{Tok, intern};
use orchid_base::location::Pos;
use orchid_base::logging::Logger;
use orchid_base::macros::mtreev_from_api;
use orchid_base::parse::Comment;
use orchid_base::reqnot::{ReqNot, Requester as _};
use orchid_base::tree::{AtomRepr, ttv_from_api};
use ordered_float::NotNan;
use substack::{Stackframe, Substack};

use crate::api;
use crate::expr::Expr;
use crate::macros::{macro_recur, macro_treev_to_api};
use crate::tree::{Member, ParsTokTree};

#[derive(Debug, destructure)]
pub struct AtomData {
	owner: System,
	drop: Option<api::AtomId>,
	data: Vec<u8>,
}
impl AtomData {
	fn api(self) -> api::Atom {
		let (owner, drop, data) = self.destructure();
		api::Atom { data, drop, owner: owner.id() }
	}
	fn api_ref(&self) -> api::Atom {
		api::Atom { data: self.data.clone(), drop: self.drop, owner: self.owner.id() }
	}
}
impl Drop for AtomData {
	fn drop(&mut self) {
		if let Some(id) = self.drop {
			self.owner.reqnot().notify(api::AtomDrop(self.owner.id(), id))
		}
	}
}

#[derive(Clone, Debug)]
pub struct AtomHand(Arc<AtomData>);
impl AtomHand {
	pub fn from_api(atom: api::Atom) -> Self {
		fn create_new(api::Atom { data, drop, owner }: api::Atom) -> AtomHand {
			let owner = System::resolve(owner).expect("Atom owned by non-existing system");
			AtomHand(Arc::new(AtomData { data, drop, owner }))
		}
		if let Some(id) = atom.drop {
			lazy_static! {
				static ref OWNED_ATOMS: Mutex<HashMap<(api::SysId, api::AtomId), Weak<AtomData>>> =
					Mutex::default();
			}
			let owner = atom.owner;
			let mut owned_g = OWNED_ATOMS.lock().unwrap();
			if let Some(data) = owned_g.get(&(owner, id)) {
				if let Some(atom) = data.upgrade() {
					return Self(atom);
				}
			}
			let new = create_new(atom);
			owned_g.insert((owner, id), Arc::downgrade(&new.0));
			new
		} else {
			create_new(atom)
		}
	}
	pub fn call(self, arg: Expr) -> api::Expression {
		let owner_sys = self.0.owner.clone();
		let reqnot = owner_sys.reqnot();
		let ticket = owner_sys.give_expr(arg.canonicalize(), || arg);
		match Arc::try_unwrap(self.0) {
			Ok(data) => reqnot.request(api::FinalCall(data.api(), ticket)),
			Err(hand) => reqnot.request(api::CallRef(hand.api_ref(), ticket)),
		}
	}
	pub fn req(&self, key: api::TStrv, req: Vec<u8>) -> Option<Vec<u8>> {
		self.0.owner.reqnot().request(api::Fwded(self.0.api_ref(), key, req))
	}
	pub fn api_ref(&self) -> api::Atom { self.0.api_ref() }
	pub fn print(&self) -> String { self.0.owner.reqnot().request(api::AtomPrint(self.0.api_ref())) }
}
impl AtomRepr for AtomHand {
	type Ctx = ();
	fn from_api(atom: &orchid_api::Atom, _: Pos, (): &mut Self::Ctx) -> Self {
		Self::from_api(atom.clone())
	}
	fn to_api(&self) -> orchid_api::Atom { self.api_ref() }
}
impl fmt::Display for AtomHand {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.print()) }
}

/// Data held about an Extension. This is refcounted within [Extension]. It's
/// important to only ever access parts of this struct through the [Arc] because
/// the components reference each other through [Weak]s of it, and will panic if
/// upgrading fails.
#[derive(destructure)]
pub struct ExtensionData {
	port: Mutex<Box<dyn ExtPort>>,
	// child: Mutex<process::Child>,
	// child_stdin: Mutex<ChildStdin>,
	reqnot: ReqNot<api::HostMsgSet>,
	systems: Vec<SystemCtor>,
	logger: Logger,
}
impl Drop for ExtensionData {
	fn drop(&mut self) { self.reqnot.notify(api::HostExtNotif::Exit); }
}

fn acq_expr(sys: api::SysId, extk: api::ExprTicket) {
	(System::resolve(sys).expect("Expr acq'd by invalid system"))
		.give_expr(extk, || Expr::resolve(extk).expect("Invalid expr acq'd"));
}

fn rel_expr(sys: api::SysId, extk: api::ExprTicket) {
	let sys = System::resolve(sys).unwrap();
	let mut exprs = sys.0.exprs.write().unwrap();
	exprs.entry(extk).and_replace_entry_with(|_, (rc, rt)| {
		(0 < rc.fetch_sub(1, Ordering::Relaxed)).then_some((rc, rt))
	});
}

#[derive(Clone)]
pub struct Extension(Arc<ExtensionData>);
impl Extension {
	pub fn new(fac: Box<dyn ExtFactory>, logger: Logger) -> io::Result<Self> {
		Ok(Self(Arc::new_cyclic(|weak: &Weak<ExtensionData>| {
			let (eh, port) = fac.run(Box::new(clone!(weak; move |msg| {
				weak.upgrade().inspect(|xd| xd.reqnot.receive(msg));
			})));
			ExtensionData {
				systems: (eh.systems.iter().cloned())
					.map(|decl| SystemCtor { decl, ext: weak.clone() })
					.collect(),
				logger,
				port: Mutex::new(port),
				reqnot: ReqNot::new(
					clone!(weak; move |sfn, _| {
						let data = weak.upgrade().unwrap();
						data.logger.log_buf("Downsending", sfn);
						data.port.lock().unwrap().send(sfn);
					}),
					clone!(weak; move |notif, _| match notif {
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Acquire(acq)) => acq_expr(acq.0, acq.1),
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Release(rel)) => rel_expr(rel.0, rel.1),
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Move(mov)) => {
							acq_expr(mov.inc, mov.expr);
							rel_expr(mov.dec, mov.expr);
						},
						api::ExtHostNotif::Log(api::Log(str)) => weak.upgrade().unwrap().logger.log(str),
					}),
					|hand, req| match req {
						api::ExtHostReq::Ping(ping) => hand.handle(&ping, &()),
						api::ExtHostReq::IntReq(intreq) => match intreq {
							api::IntReq::InternStr(s) => hand.handle(&s, &intern(&**s.0).to_api()),
							api::IntReq::InternStrv(v) => hand.handle(&v, &intern(&*v.0).to_api()),
							api::IntReq::ExternStr(si) => hand.handle(&si, &Tok::<String>::from_api(si.0).arc()),
							api::IntReq::ExternStrv(vi) => hand.handle(
								&vi,
								&Arc::new(
									Tok::<Vec<Tok<String>>>::from_api(vi.0).iter().map(|t| t.to_api()).collect_vec(),
								),
							),
						},
						api::ExtHostReq::Fwd(ref fw @ api::Fwd(ref atom, ref key, ref body)) => {
							let sys = System::resolve(atom.owner).unwrap();
							hand.handle(fw, &sys.reqnot().request(api::Fwded(fw.0.clone(), *key, body.clone())))
						},
						api::ExtHostReq::SysFwd(ref fw @ api::SysFwd(id, ref body)) => {
							let sys = System::resolve(id).unwrap();
							hand.handle(fw, &sys.request(body.clone()))
						},
						api::ExtHostReq::SubLex(sl) => {
							let (rep_in, rep_out) = sync_channel(0);
							let lex_g = LEX_RECUR.lock().unwrap();
							let req_in = lex_g.get(&sl.id).expect("Sublex for nonexistent lexid");
							req_in.send(ReqPair(sl.clone(), rep_in)).unwrap();
							hand.handle(&sl, &rep_out.recv().unwrap())
						},
						api::ExtHostReq::ExprReq(api::ExprReq::Inspect(ins @ api::Inspect { target })) => {
							let expr = Expr::resolve(target).expect("Invalid ticket");
							hand.handle(&ins, &api::Inspected {
								refcount: expr.strong_count() as u32,
								location: expr.pos().to_api(),
								kind: expr.to_api(),
							})
						},
						api::ExtHostReq::RunMacros(ref rm @ api::RunMacros { ref run_id, ref query }) => hand
							.handle(
								rm,
								&macro_recur(
									*run_id,
									mtreev_from_api(query, &mut |_| panic!("Recursion never contains atoms")),
								)
								.map(|x| macro_treev_to_api(*run_id, x)),
							),
					},
				),
			}
		})))
	}
	pub fn systems(&self) -> impl Iterator<Item = &SystemCtor> { self.0.systems.iter() }
}

pub struct SystemCtor {
	decl: api::SystemDecl,
	ext: Weak<ExtensionData>,
}
impl SystemCtor {
	pub fn name(&self) -> &str { &self.decl.name }
	pub fn priority(&self) -> NotNan<f64> { self.decl.priority }
	pub fn depends(&self) -> impl ExactSizeIterator<Item = &str> {
		self.decl.depends.iter().map(|s| &**s)
	}
	pub fn run<'a>(&self, depends: impl IntoIterator<Item = &'a System>) -> System {
		let mut inst_g = SYSTEM_INSTS.write().unwrap();
		let depends = depends.into_iter().map(|si| si.id()).collect_vec();
		debug_assert_eq!(depends.len(), self.decl.depends.len(), "Wrong number of deps provided");
		let ext = self.ext.upgrade().expect("SystemCtor should be freed before Extension");
		static NEXT_ID: AtomicU16 = AtomicU16::new(1);
		let id =
			api::SysId(NonZero::new(NEXT_ID.fetch_add(1, Ordering::Relaxed)).expect("next_id wrapped"));
		let sys_inst = ext.reqnot.request(api::NewSystem { depends, id, system: self.decl.id });
		let data = System(Arc::new(SystemInstData {
			decl_id: self.decl.id,
			ext: Extension(ext),
			exprs: RwLock::default(),
			lex_filter: sys_inst.lex_filter,
			const_root: OnceLock::new(),
			line_types: sys_inst.line_types.into_iter().map(Tok::from_api).collect(),
			id,
		}));
		let root = (sys_inst.const_root.into_iter())
			.map(|(k, v)| {
				Member::from_api(
					api::Member { name: k, kind: v },
					Substack::Bottom.push(Tok::from_api(k)),
					&data,
				)
			})
			.collect_vec();
		data.0.const_root.set(root).unwrap();
		inst_g.insert(id, data.clone());
		data
	}
}

lazy_static! {
	static ref SYSTEM_INSTS: RwLock<HashMap<api::SysId, System>> = RwLock::default();
	static ref LEX_RECUR: Mutex<HashMap<api::ParsId, SyncSender<ReqPair<api::SubLex>>>> =
		Mutex::default();
}

pub struct ReqPair<R: Request>(R, pub SyncSender<R::Response>);

#[derive(destructure)]
pub struct SystemInstData {
	exprs: RwLock<HashMap<api::ExprTicket, (AtomicU32, Expr)>>,
	ext: Extension,
	decl_id: api::SysDeclId,
	lex_filter: api::CharFilter,
	id: api::SysId,
	const_root: OnceLock<Vec<Member>>,
	line_types: Vec<Tok<String>>,
}
impl Drop for SystemInstData {
	fn drop(&mut self) {
		self.ext.0.reqnot.notify(api::SystemDrop(self.id));
		if let Ok(mut g) = SYSTEM_INSTS.write() {
			g.remove(&self.id);
		}
	}
}
#[derive(Clone)]
pub struct System(Arc<SystemInstData>);
impl System {
	pub fn id(&self) -> api::SysId { self.id }
	fn resolve(id: api::SysId) -> Option<System> { SYSTEM_INSTS.read().unwrap().get(&id).cloned() }
	fn reqnot(&self) -> &ReqNot<api::HostMsgSet> { &self.0.ext.0.reqnot }
	fn give_expr(&self, ticket: api::ExprTicket, get_expr: impl FnOnce() -> Expr) -> api::ExprTicket {
		match self.0.exprs.write().unwrap().entry(ticket) {
			Entry::Occupied(mut oe) => {
				oe.get_mut().0.fetch_add(1, Ordering::Relaxed);
			},
			Entry::Vacant(v) => {
				v.insert((AtomicU32::new(1), get_expr()));
			},
		}
		ticket
	}
	pub fn get_tree(&self, id: api::TreeId) -> api::MemberKind {
		self.reqnot().request(api::GetMember(self.0.id, id))
	}
	pub fn has_lexer(&self) -> bool { !self.0.lex_filter.0.is_empty() }
	pub fn can_lex(&self, c: char) -> bool { char_filter_match(&self.0.lex_filter, c) }
	/// Have this system lex a part of the source. It is assumed that
	/// [Self::can_lex] was called and returned true.
	pub fn lex(
		&self,
		source: Tok<String>,
		pos: u32,
		mut r: impl FnMut(u32) -> Option<api::SubLexed> + Send,
	) -> api::OrcResult<Option<api::LexedExpr>> {
		// get unique lex ID
		static LEX_ID: AtomicU64 = AtomicU64::new(1);
		let id = api::ParsId(NonZero::new(LEX_ID.fetch_add(1, Ordering::Relaxed)).unwrap());
		thread::scope(|s| {
			// create and register channel
			let (req_in, req_out) = sync_channel(0);
			LEX_RECUR.lock().unwrap().insert(id, req_in); // LEX_RECUR released
			// spawn recursion handler which will exit when the sender is collected
			s.spawn(move || {
				while let Ok(ReqPair(sublex, rep_in)) = req_out.recv() {
					rep_in.send(r(sublex.pos)).unwrap()
				}
			});
			// Pass control to extension
			let ret =
				self.reqnot().request(api::LexExpr { id, pos, sys: self.id(), text: source.to_api() });
			// collect sender to unblock recursion handler thread before returning
			LEX_RECUR.lock().unwrap().remove(&id);
			ret.transpose()
		}) // exit recursion handler thread
	}
	pub fn can_parse(&self, line_type: Tok<String>) -> bool { self.line_types.contains(&line_type) }
	pub fn line_types(&self) -> impl Iterator<Item = Tok<String>> + '_ {
		self.line_types.iter().cloned()
	}
	pub fn parse(
		&self,
		line: Vec<ParsTokTree>,
		exported: bool,
		comments: Vec<Comment>,
	) -> OrcRes<Vec<ParsTokTree>> {
		let line = line.iter().map(|t| t.to_api(&mut |n, _| match *n {})).collect_vec();
		let comments = comments.iter().map(Comment::to_api).collect_vec();
		let parsed =
			(self.reqnot().request(api::ParseLine { exported, sys: self.id(), comments, line }))
				.map_err(|e| OrcErrv::from_api(&e))?;
		Ok(ttv_from_api(parsed, &mut ()))
	}
	pub fn request(&self, req: Vec<u8>) -> Vec<u8> {
		self.reqnot().request(api::SysFwded(self.id(), req))
	}
}
impl fmt::Debug for System {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let ctor = (self.0.ext.0.systems.iter().find(|c| c.decl.id == self.0.decl_id))
			.expect("System instance with no associated constructor");
		write!(f, "System({} @ {} #{})", ctor.decl.name, ctor.decl.priority, self.0.id.0)?;
		match self.0.exprs.read() {
			Err(_) => write!(f, "expressions unavailable"),
			Ok(r) => {
				let rc: u32 = r.values().map(|v| v.0.load(Ordering::Relaxed)).sum();
				write!(f, "{rc} refs to {} exprs", r.len())
			},
		}
	}
}
impl Deref for System {
	type Target = SystemInstData;
	fn deref(&self) -> &Self::Target { self.0.as_ref() }
}

#[derive(Debug, Clone)]
pub enum SysResolvErr {
	Loop(Vec<String>),
	Missing(String),
}

pub fn init_systems(tgts: &[String], exts: &[Extension]) -> Result<Vec<System>, SysResolvErr> {
	let mut to_load = HashMap::<&str, &SystemCtor>::new();
	let mut to_find = tgts.iter().map(|s| s.as_str()).collect::<VecDeque<&str>>();
	while let Some(target) = to_find.pop_front() {
		if to_load.contains_key(target) {
			continue;
		}
		let ctor = (exts.iter())
			.flat_map(|e| e.systems().filter(|c| c.decl.name == target))
			.max_by_key(|c| c.decl.priority)
			.ok_or_else(|| SysResolvErr::Missing(target.to_string()))?;
		to_load.insert(target, ctor);
		to_find.extend(ctor.decl.depends.iter().map(|s| s.as_str()));
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
			for dep in ctor.decl.depends.iter() {
				if Substack::Frame(chain).iter().any(|c| c == dep) {
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
		let sys = ctor.run(ctor.depends().map(|n| &systems[n]));
		systems.insert(ctor.name(), sys);
	}
	Ok(systems.into_values().collect_vec())
}
