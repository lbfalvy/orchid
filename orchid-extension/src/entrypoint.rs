use std::cell::RefCell;
use std::future::Future;
use std::mem;
use std::num::NonZero;
use std::pin::Pin;
use std::rc::Rc;

use async_std::channel::{self, Receiver, RecvError, Sender};
use async_std::stream;
use async_std::sync::Mutex;
use futures::future::{LocalBoxFuture, join_all};
use futures::{FutureExt, StreamExt, stream_select};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::{ExtMsgSet, IntReq};
use orchid_api_traits::{Decode, UnderRoot, enc_vec};
use orchid_base::builtin::{ExtInit, ExtPort, Spawner};
use orchid_base::char_filter::{char_filter_match, char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::interner::{Interner, Tok};
use orchid_base::logging::Logger;
use orchid_base::name::Sym;
use orchid_base::parse::{Comment, Snippet};
use orchid_base::reqnot::{ReqNot, RequestHandle, Requester};
use orchid_base::tree::{TokenVariant, ttv_from_api, ttv_into_api};
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo, AtomTypeId};
use crate::atom_owned::take_atom;
use crate::expr::{Expr, ExprHandle};
use crate::fs::VirtFS;
use crate::lexer::{LexContext, err_cascade, err_not_applicable};
use crate::system::{SysCtx, atom_by_idx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::{GenItemKind, GenTok, GenTokTree, LazyMemberFactory, TreeIntoApiCtxImpl};

pub type ExtReq<'a> = RequestHandle<'a, api::ExtMsgSet>;
pub type ExtReqNot = ReqNot<api::ExtMsgSet>;

pub struct ExtensionData {
	pub name: &'static str,
	pub systems: &'static [&'static dyn DynSystemCtor],
}
impl ExtensionData {
	pub fn new(name: &'static str, systems: &'static [&'static dyn DynSystemCtor]) -> Self {
		Self { name, systems }
	}
	// pub fn main(self) { extension_main(self) }
}

pub enum MemberRecord {
	Gen(Vec<Tok<String>>, LazyMemberFactory),
	Res,
}

pub struct SystemRecord {
	vfses: HashMap<api::VfsId, &'static dyn VirtFS>,
	declfs: api::EagerVfs,
	lazy_members: HashMap<api::TreeId, MemberRecord>,
	ctx: SysCtx,
}

trait_set! {
	pub trait WithAtomRecordCallback<'a, T> = AsyncFnOnce(
		Box<dyn AtomDynfo>,
		SysCtx,
		AtomTypeId,
		&'a [u8]
	) -> T
}

pub async fn with_atom_record<'a, F: Future<Output = SysCtx>, T>(
	get_sys_ctx: &impl Fn(api::SysId) -> F,
	atom: &'a api::Atom,
	cb: impl WithAtomRecordCallback<'a, T>,
) -> T {
	let mut data = &atom.data[..];
	let ctx = get_sys_ctx(atom.owner).await;
	let inst = ctx.get::<CtedObj>().inst();
	let id = AtomTypeId::decode(Pin::new(&mut data)).await;
	let atom_record = atom_by_idx(inst.card(), id.clone()).expect("Atom ID reserved");
	cb(atom_record, ctx, id, data).await
}

// pub fn extension_main(data: ExtensionData) {

// 	if thread::Builder::new()
// 		.name(format!("ext-main:{}", data.name))
// 		.spawn(|| extension_main_logic(data))
// 		.unwrap()
// 		.join()
// 		.is_err()
// 	{
// 		process::exit(-1)
// 	}
// }

pub struct ExtensionOwner {
	_interner_cell: Rc<RefCell<Option<Interner>>>,
	_systems_lock: Rc<Mutex<HashMap<api::SysId, SystemRecord>>>,
	out_recv: Receiver<Vec<u8>>,
	out_send: Sender<Vec<u8>>,
}

impl ExtPort for ExtensionOwner {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()> {
		Box::pin(async { self.out_send.send(msg.to_vec()).boxed_local().await.unwrap() })
	}
	fn recv(&self) -> LocalBoxFuture<'_, Option<Vec<u8>>> {
		Box::pin(async {
			match self.out_recv.recv().await {
				Ok(v) => Some(v),
				Err(RecvError) => None,
			}
		})
	}
}

pub fn extension_init(
	data: ExtensionData,
	host_header: api::HostHeader,
	spawner: Spawner,
) -> ExtInit {
	let api::HostHeader { log_strategy, msg_logs } = host_header;
	let decls = (data.systems.iter().enumerate())
		.map(|(id, sys)| (u16::try_from(id).expect("more than u16max system ctors"), sys))
		.map(|(id, sys)| sys.decl(api::SysDeclId(NonZero::new(id + 1).unwrap())))
		.collect_vec();
	let systems_lock = Rc::new(Mutex::new(HashMap::<api::SysId, SystemRecord>::new()));
	let ext_header = api::ExtensionHeader { name: data.name.to_string(), systems: decls.clone() };
	let (out_send, in_recv) = channel::bounded::<Vec<u8>>(1);
	let (in_send, out_recv) = channel::bounded::<Vec<u8>>(1);
	let (exit_send, exit_recv) = channel::bounded(1);
	let logger = Logger::new(log_strategy);
	let msg_logger = Logger::new(msg_logs);
	let interner_cell = Rc::new(RefCell::new(None::<Interner>));
	let interner_weak = Rc::downgrade(&interner_cell);
	let systems_weak = Rc::downgrade(&systems_lock);
	let get_ctx = clone!(systems_weak; move |id: api::SysId| clone!(systems_weak; async move {
		let systems =
			systems_weak.upgrade().expect("System table dropped before request processing done");
		let x = systems.lock().await.get(&id).expect("System not found").ctx.clone();
		x
	}));
	let init_ctx = {
		clone!(interner_weak, spawner, logger);
		move |id: api::SysId, cted: CtedObj, reqnot: ReqNot<ExtMsgSet>| {
			clone!(interner_weak, spawner, logger; async move {
				let interner_rc =
					interner_weak.upgrade().expect("System construction order while shutting down");
				let i = interner_rc.borrow().clone().expect("mk_ctx called very early, no interner!");
				SysCtx::new(id, i, reqnot, spawner, logger, cted)
			})
		}
	};
	let rn = ReqNot::<api::ExtMsgSet>::new(
		msg_logger.clone(),
		move |a, _| clone!(in_send; Box::pin(async move { in_send.send(a.to_vec()).await.unwrap() })),
		clone!(systems_weak, exit_send, get_ctx; move |n, _| {
			clone!(systems_weak, exit_send, get_ctx; async move {
				match n {
					api::HostExtNotif::Exit => exit_send.send(()).await.unwrap(),
					api::HostExtNotif::SystemDrop(api::SystemDrop(sys_id)) =>
						if let Some(rc) = systems_weak.upgrade() {
							mem::drop(rc.lock().await.remove(&sys_id))
						},
					api::HostExtNotif::AtomDrop(api::AtomDrop(sys_id, atom)) => {
						let ctx = get_ctx(sys_id).await;
						take_atom(atom, &ctx).await.dyn_free(ctx.clone()).await
					}
				}
			}.boxed_local())
		}),
		{
			clone!(logger, get_ctx, init_ctx, systems_weak, interner_weak, decls, msg_logger);
			move |hand, req| {
				clone!(logger, get_ctx, init_ctx, systems_weak, interner_weak, decls, msg_logger);
				async move {
					let interner_cell = interner_weak.upgrade().expect("Interner dropped before request");
					let i = interner_cell.borrow().clone().expect("Request arrived before interner set");
					writeln!(msg_logger, "{} extension received request {req:?}", data.name);
					match req {
						api::HostExtReq::Ping(ping @ api::Ping) => hand.handle(&ping, &()).await,
						api::HostExtReq::Sweep(sweep @ api::Sweep) =>
							hand.handle(&sweep, &i.sweep_replica().await).await,
						api::HostExtReq::SysReq(api::SysReq::NewSystem(new_sys)) => {
							let (sys_id, _) = (decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system))
								.expect("NewSystem call received for invalid system");
							let cted = data.systems[sys_id].new_system(&new_sys);
							let mut vfses = HashMap::new();
							let lex_filter =
								cted.inst().dyn_lexers().iter().fold(api::CharFilter(vec![]), |cf, lx| {
									char_filter_union(&cf, &mk_char_filter(lx.char_filter().iter().cloned()))
								});
							let lazy_mems = Mutex::new(HashMap::new());
							let ctx = init_ctx(new_sys.id, cted.clone(), hand.reqnot()).await;
							let const_root = stream::from_iter(cted.inst().dyn_env())
								.filter_map(
									async |i| if let GenItemKind::Member(m) = i.kind { Some(m) } else { None },
								)
								.then(|mem| {
									let (req, lazy_mems) = (&hand, &lazy_mems);
									clone!(i, ctx; async move {
										let name = i.i(&mem.name).await.to_api();
										let value = mem.kind.into_api(&mut TreeIntoApiCtxImpl {
											lazy_members: &mut *lazy_mems.lock().await,
											sys: ctx,
											basepath: &[],
											path: Substack::Bottom,
											req
										})
										.await;
										(name, value)
									})
								})
								.collect()
								.await;
							let declfs = cted.inst().dyn_vfs().to_api_rec(&mut vfses, &i).await;
							let record =
								SystemRecord { declfs, vfses, ctx, lazy_members: lazy_mems.into_inner() };
							let systems = systems_weak.upgrade().expect("System constructed during shutdown");
							systems.lock().await.insert(new_sys.id, record);
							hand
								.handle(&new_sys, &api::NewSystemResponse {
									lex_filter,
									const_root,
									line_types: vec![],
								})
								.await
						},
						api::HostExtReq::GetMember(get_tree @ api::GetMember(sys_id, tree_id)) => {
							let sys_ctx = get_ctx(sys_id).await;
							let systems = systems_weak.upgrade().expect("Member queried during shutdown");
							let mut systems_g = systems.lock().await;
							let SystemRecord { lazy_members, .. } =
								systems_g.get_mut(&sys_id).expect("System not found");
							let (path, cb) = match lazy_members.insert(tree_id, MemberRecord::Res) {
								None => panic!("Tree for ID not found"),
								Some(MemberRecord::Res) => panic!("This tree has already been transmitted"),
								Some(MemberRecord::Gen(path, cb)) => (path, cb),
							};
							let tree = cb.build(Sym::new(path.clone(), &i).await.unwrap(), sys_ctx.clone()).await;
							let mut tia_ctx = TreeIntoApiCtxImpl {
								sys: sys_ctx,
								path: Substack::Bottom,
								basepath: &path,
								lazy_members,
								req: &hand,
							};
							hand.handle(&get_tree, &tree.into_api(&mut tia_ctx).await).await
						},
						api::HostExtReq::VfsReq(api::VfsReq::GetVfs(get_vfs @ api::GetVfs(sys_id))) => {
							let systems = systems_weak.upgrade().expect("VFS root requested during shutdown");
							let systems_g = systems.lock().await;
							hand.handle(&get_vfs, &systems_g[&sys_id].declfs).await
						},
						api::HostExtReq::SysReq(api::SysReq::SysFwded(fwd)) => {
							let api::SysFwded(sys_id, payload) = fwd;
							let ctx = get_ctx(sys_id).await;
							let sys = ctx.cted().inst();
							sys.dyn_request(hand, payload).await
						},
						api::HostExtReq::VfsReq(api::VfsReq::VfsRead(vfs_read)) => {
							let api::VfsRead(sys_id, vfs_id, path) = &vfs_read;
							let ctx = get_ctx(*sys_id).await;
							let systems = systems_weak.upgrade().expect("VFS requested during shutdoown");
							let systems_g = systems.lock().await;
							let path = join_all(path.iter().map(|t| Tok::from_api(*t, &i))).await;
							let vfs = systems_g[sys_id].vfses[vfs_id].load(&path, ctx).await;
							hand.handle(&vfs_read, &vfs).await
						},
						api::HostExtReq::LexExpr(lex @ api::LexExpr { sys, text, pos, id }) => {
							let sys_ctx = get_ctx(sys).await;
							let text = Tok::from_api(text, &i).await;
							let ctx = LexContext { id, pos, text: &text, ctx: sys_ctx.clone() };
							let trigger_char = text.chars().nth(pos as usize).unwrap();
							let err_na = err_not_applicable(&i).await;
							let err_cascade = err_cascade(&i).await;
							let lexers = sys_ctx.cted().inst().dyn_lexers();
							for lx in lexers.iter().filter(|l| char_filter_match(l.char_filter(), trigger_char)) {
								match lx.lex(&text[pos as usize..], &ctx).await {
									Err(e) if e.any(|e| *e == err_na) => continue,
									Err(e) => {
										let eopt = e.keep_only(|e| *e != err_cascade).map(|e| Err(e.to_api()));
										return hand.handle(&lex, &eopt).await;
									},
									Ok((s, expr)) => {
										let expr = expr.into_api(&mut (), &mut (sys_ctx, &hand)).await;
										let pos = (text.len() - s.len()) as u32;
										return hand.handle(&lex, &Some(Ok(api::LexedExpr { pos, expr }))).await;
									},
								}
							}
							writeln!(logger, "Got notified about n/a character '{trigger_char}'");
							hand.handle(&lex, &None).await
						},
						api::HostExtReq::ParseLine(pline) => {
							let api::ParseLine { module, exported, comments, sys, line } = &pline;
							let mut ctx = get_ctx(*sys).await;
							let parsers = ctx.cted().inst().dyn_parsers();
							let comments = join_all(comments.iter().map(|c| Comment::from_api(c, &i))).await;
							let line: Vec<GenTokTree> = ttv_from_api(line, &mut ctx, &mut (), &i).await;
							let snip = Snippet::new(line.first().expect("Empty line"), &line);
							let (head, tail) = snip.pop_front().unwrap();
							let name = if let GenTok::Name(n) = &head.tok { n } else { panic!("No line head") };
							let parser =
								parsers.iter().find(|p| p.line_head() == **name).expect("No parser candidate");
							let module = Sym::from_api(*module, ctx.i()).await;
							let o_line = match parser.parse(ctx.clone(), module, *exported, comments, tail).await
							{
								Err(e) => Err(e.to_api()),
								Ok(t) => Ok(ttv_into_api(t, &mut (), &mut (ctx.clone(), &hand)).await),
							};
							hand.handle(&pline, &o_line).await
						},
						api::HostExtReq::AtomReq(atom_req) => {
							let atom = atom_req.get_atom();
							let atom_req = atom_req.clone();
							with_atom_record(&get_ctx, atom, async move |nfo, ctx, id, buf| {
								let actx = AtomCtx(buf, atom.drop, ctx.clone());
								match &atom_req {
									api::AtomReq::SerializeAtom(ser) => {
										let mut buf = enc_vec(&id).await;
										match nfo.serialize(actx, Pin::<&mut Vec<_>>::new(&mut buf)).await {
											None => hand.handle(ser, &None).await,
											Some(refs) => {
												let refs =
													join_all(refs.into_iter().map(|ex| async { ex.into_api(&mut ()).await }))
														.await;
												hand.handle(ser, &Some((buf, refs))).await
											},
										}
									},
									api::AtomReq::AtomPrint(print @ api::AtomPrint(_)) =>
										hand.handle(print, &nfo.print(actx).await.to_api()).await,
									api::AtomReq::Fwded(fwded) => {
										let api::Fwded(_, key, payload) = &fwded;
										let mut reply = Vec::new();
										let key = Sym::from_api(*key, &i).await;
										let some = nfo
											.handle_req(
												actx,
												key,
												Pin::<&mut &[u8]>::new(&mut &payload[..]),
												Pin::<&mut Vec<_>>::new(&mut reply),
											)
											.await;
										hand.handle(fwded, &some.then_some(reply)).await
									},
									api::AtomReq::CallRef(call @ api::CallRef(_, arg)) => {
										// SAFETY: function calls own their argument implicitly
										let expr_handle = unsafe { ExprHandle::from_args(ctx.clone(), *arg) };
										let ret = nfo.call_ref(actx, Expr::from_handle(Rc::new(expr_handle))).await;
										hand.handle(call, &ret.api_return(ctx.clone(), &hand).await).await
									},
									api::AtomReq::FinalCall(call @ api::FinalCall(_, arg)) => {
										// SAFETY: function calls own their argument implicitly
										let expr_handle = unsafe { ExprHandle::from_args(ctx.clone(), *arg) };
										let ret = nfo.call(actx, Expr::from_handle(Rc::new(expr_handle))).await;
										hand.handle(call, &ret.api_return(ctx.clone(), &hand).await).await
									},
									api::AtomReq::Command(cmd @ api::Command(_)) => match nfo.command(actx).await {
										Err(e) => hand.handle(cmd, &Err(e.to_api())).await,
										Ok(opt) => match opt {
											None => hand.handle(cmd, &Ok(api::NextStep::Halt)).await,
											Some(cont) => {
												let cont = cont.api_return(ctx.clone(), &hand).await;
												hand.handle(cmd, &Ok(api::NextStep::Continue(cont))).await
											},
										},
									},
								}
							})
							.await
						},
						api::HostExtReq::DeserAtom(deser) => {
							let api::DeserAtom(sys, buf, refs) = &deser;
							let mut read = &mut &buf[..];
							let ctx = get_ctx(*sys).await;
							// SAFETY: deserialization implicitly grants ownership to previously owned exprs
							let refs = (refs.iter())
								.map(|tk| unsafe { ExprHandle::from_args(ctx.clone(), *tk) })
								.map(|handle| Expr::from_handle(Rc::new(handle)))
								.collect_vec();
							let id = AtomTypeId::decode(Pin::new(&mut read)).await;
							let inst = ctx.cted().inst();
							let nfo = atom_by_idx(inst.card(), id).expect("Deserializing atom with invalid ID");
							hand.handle(&deser, &nfo.deserialize(ctx.clone(), read, &refs).await).await
						},
					}
				}
				.boxed_local()
			}
		},
	);
	*interner_cell.borrow_mut() =
		Some(Interner::new_replica(rn.clone().map(|ir: IntReq| ir.into_root())));
	spawner(Box::pin(clone!(spawner; async move {
		let mut streams = stream_select! { in_recv.map(Some), exit_recv.map(|_| None) };
		while let Some(item) = streams.next().await {
			match item {
				Some(rcvd) => spawner(Box::pin(clone!(rn; async move { rn.receive(&rcvd[..]).await }))),
				None => break,
			}
		}
	})));
	ExtInit {
		header: ext_header,
		port: Box::new(ExtensionOwner {
			out_recv,
			out_send,
			_interner_cell: interner_cell,
			_systems_lock: systems_lock,
		}),
	}
}
