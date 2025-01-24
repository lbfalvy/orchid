use std::cell::RefCell;
use std::future::Future;
use std::io::Write;
use std::mem;
use std::num::NonZero;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_std::channel::{Receiver, Sender};
use async_std::stream;
use async_std::sync::Mutex;
use futures::future::{LocalBoxFuture, join_all};
use futures::task::LocalSpawn;
use futures::{FutureExt, StreamExt};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::ApplyMacro;
use orchid_api_traits::{Decode, Encode, enc_vec};
use orchid_base::builtin::ExtPort;
use orchid_base::char_filter::{char_filter_match, char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::interner::{Interner, Tok};
use orchid_base::logging::Logger;
use orchid_base::macros::{mtreev_from_api, mtreev_to_api};
use orchid_base::name::{PathSlice, Sym};
use orchid_base::parse::{Comment, Snippet};
use orchid_base::reqnot::{ReqNot, RequestHandle, Requester};
use orchid_base::tree::{ttv_from_api, ttv_to_api};
use substack::Substack;
use trait_set::trait_set;

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo};
use crate::atom_owned::ObjStore;
use crate::fs::VirtFS;
use crate::lexer::{LexContext, err_cascade, err_not_applicable};
use crate::macros::{Rule, RuleCtx};
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::{SysCtx, atom_by_idx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::{GenTok, GenTokTree, LazyMemberFactory, TIACtxImpl, do_extra};

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
	cted: CtedObj,
	vfses: HashMap<api::VfsId, &'static dyn VirtFS>,
	declfs: api::EagerVfs,
	lazy_members: HashMap<api::TreeId, MemberRecord>,
	rules: HashMap<api::MacroId, Rc<Rule>>,
}

trait_set! {
	pub trait WARCallback<'a, T> = FnOnce(
		Box<dyn AtomDynfo>,
		SysCtx,
		api::AtomId,
		&'a [u8]
	) -> LocalBoxFuture<'a, T>
}

pub async fn with_atom_record<'a, F: Future<Output = SysCtx>, T>(
	get_sys_ctx: &impl Fn(api::SysId, ReqNot<api::ExtMsgSet>) -> F,
	reqnot: ReqNot<api::ExtMsgSet>,
	atom: &'a api::Atom,
	cb: impl WARCallback<'a, T>,
) -> T {
	let mut data = &atom.data[..];
	let ctx = get_sys_ctx(atom.owner, reqnot).await;
	let inst = ctx.cted.inst();
	let id = api::AtomId::decode(Pin::new(&mut data)).await;
	let atom_record = atom_by_idx(inst.card(), id).expect("Atom ID reserved");
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
	rn: ReqNot<api::ExtMsgSet>,
	out_recv: Receiver<Vec<u8>>,
	out_send: Sender<Vec<u8>>,
}

impl ExtPort for ExtensionOwner {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()> {
		self.rn.receive(msg).boxed_local()
	}
	fn recv<'a>(
		&'a self,
		cb: Box<dyn FnOnce(&[u8]) -> LocalBoxFuture<'_, ()> + 'a>,
	) -> LocalBoxFuture<'a, ()> {
		async {
			let msg = self.out_recv.recv().await.unwrap();
			cb(&msg[..]).await
		}
		.boxed_local()
	}
}

async fn extension_main_logic(data: ExtensionData, spawner: Rc<dyn LocalSpawn>) {
	let api::HostHeader { log_strategy } =
		api::HostHeader::decode(Pin::new(&mut async_std::io::stdin())).await;
	let mut buf = Vec::new();
	let decls = (data.systems.iter().enumerate())
		.map(|(id, sys)| (u16::try_from(id).expect("more than u16max system ctors"), sys))
		.map(|(id, sys)| sys.decl(api::SysDeclId(NonZero::new(id + 1).unwrap())))
		.collect_vec();
	let systems = Rc::new(Mutex::new(HashMap::<api::SysId, SystemRecord>::new()));
	api::ExtensionHeader { name: data.name.to_string(), systems: decls.clone() }
		.encode(Pin::new(&mut buf))
		.await;
	std::io::stdout().write_all(&buf).unwrap();
	std::io::stdout().flush().unwrap();
	let exiting = Arc::new(AtomicBool::new(false));
	let logger = Arc::new(Logger::new(log_strategy));
	let interner_cell = Rc::new(RefCell::new(None::<Rc<Interner>>));
	let interner_weak = Rc::downgrade(&interner_cell);
	let obj_store = ObjStore::default();
	let mk_ctx = clone!(
		logger, systems, spawner, obj_store, interner_weak;
		move |id: api::SysId, reqnot: ReqNot<api::ExtMsgSet>| {
			clone!(logger, systems, spawner, obj_store, interner_weak; async move {
				let cted = systems.lock().await[&id].cted.clone();
				let interner_cell = (interner_weak.upgrade())
					.expect("mk_ctx called after Interner rc dropped");
				let i = (interner_cell.borrow().clone())
					.expect("mk_ctx called before interner initialized");
				SysCtx { id, cted, logger, reqnot, spawner, obj_store, i: i.clone() }
			}.boxed_local())
	});
	let rn = ReqNot::<api::ExtMsgSet>::new(
		clone!(logger; move |a, _| clone!(logger; async move {
			logger.log_buf("Upsending", a);
			send_parent_msg(a).await.unwrap()
		}.boxed_local())),
		clone!(systems, exiting, mk_ctx, obj_store; move |n, reqnot| {
			clone!(systems, exiting, mk_ctx, obj_store; async move {
				match n {
					api::HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
					api::HostExtNotif::SystemDrop(api::SystemDrop(sys_id)) =>
						mem::drop(systems.lock().await.remove(&sys_id)),
					api::HostExtNotif::AtomDrop(api::AtomDrop(sys_id, atom)) =>
						obj_store.get(atom.0).unwrap().remove().dyn_free(mk_ctx(sys_id, reqnot).await).await,
				}
			}.boxed_local())
		}),
		{
			clone!(systems, logger, mk_ctx, interner_weak, obj_store, spawner, decls);
			move |hand, req| {
				clone!(systems, logger, mk_ctx, interner_weak, obj_store, spawner, decls);
				async move {
					let interner_cell = interner_weak.upgrade().expect("Interner dropped before request");
					let i = interner_cell.borrow().clone().expect("Request arrived before interner set");
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
							let rules = Mutex::new(HashMap::new());
							let ctx = SysCtx {
								cted: cted.clone(),
								id: new_sys.id,
								logger: logger.clone(),
								reqnot: hand.reqnot(),
								i: i.clone(),
								obj_store: obj_store.clone(),
								spawner: spawner.clone(),
							};
							let const_root = stream::from_iter(cted.inst().dyn_env())
								.then(|(k, v)| {
									let (req, lazy_mems, rules) = (&hand, &lazy_mems, &rules);
									clone!(i, ctx; async move {
										let name = i.i::<String>(&k).await.to_api();
										let value = v.into_api(&mut TIACtxImpl {
											lazy_members: &mut *lazy_mems.lock().await,
											rules: &mut *rules.lock().await,
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
							let record = SystemRecord {
								declfs,
								vfses,
								cted,
								lazy_members: lazy_mems.into_inner(),
								rules: rules.into_inner(),
							};
							systems.lock().await.insert(new_sys.id, record);
							hand
								.handle(&new_sys, &api::SystemInst { lex_filter, const_root, line_types: vec![] })
								.await
						},
						api::HostExtReq::GetMember(get_tree @ api::GetMember(sys_id, tree_id)) => {
							let sys_ctx = mk_ctx(sys_id, hand.reqnot()).await;
							let mut systems_g = systems.lock().await;
							let SystemRecord { lazy_members, rules, .. } =
								systems_g.get_mut(&sys_id).expect("System not found");
							let (path, cb) = match lazy_members.insert(tree_id, MemberRecord::Res) {
								None => panic!("Tree for ID not found"),
								Some(MemberRecord::Res) => panic!("This tree has already been transmitted"),
								Some(MemberRecord::Gen(path, cb)) => (path, cb),
							};
							let tree = cb.build(Sym::new(path.clone(), &i).await.unwrap()).await;
							let mut tia_ctx = TIACtxImpl {
								sys: sys_ctx,
								path: Substack::Bottom,
								basepath: &path,
								lazy_members,
								rules,
								req: &hand,
							};
							hand.handle(&get_tree, &tree.into_api(&mut tia_ctx).await).await
						},
						api::HostExtReq::VfsReq(api::VfsReq::GetVfs(get_vfs @ api::GetVfs(sys_id))) => {
							let systems_g = systems.lock().await;
							hand.handle(&get_vfs, &systems_g[&sys_id].declfs).await
						},
						api::HostExtReq::SysReq(api::SysReq::SysFwded(fwd)) => {
							let api::SysFwded(sys_id, payload) = fwd;
							let ctx = mk_ctx(sys_id, hand.reqnot()).await;
							let sys = ctx.cted.inst();
							sys.dyn_request(hand, payload).await
						},
						api::HostExtReq::VfsReq(api::VfsReq::VfsRead(vfs_read)) => {
							let api::VfsRead(sys_id, vfs_id, path) = &vfs_read;
							let ctx = mk_ctx(*sys_id, hand.reqnot()).await;
							let systems_g = systems.lock().await;
							let path = join_all(path.iter().map(|t| Tok::from_api(*t, &i))).await;
							let vfs = systems_g[sys_id].vfses[vfs_id].load(PathSlice::new(&path), ctx).await;
							hand.handle(&vfs_read, &vfs).await
						},
						api::HostExtReq::LexExpr(lex @ api::LexExpr { sys, text, pos, id }) => {
							let systems_g = systems.lock().await;
							let lexers = systems_g[&sys].cted.inst().dyn_lexers();
							mem::drop(systems_g);
							let text = Tok::from_api(text, &i).await;
							let ctx = LexContext { sys, id, pos, reqnot: hand.reqnot(), text: &text, i: &i };
							let trigger_char = text.chars().nth(pos as usize).unwrap();
							let err_na = err_not_applicable(&i).await;
							let err_cascade = err_cascade(&i).await;
							for lx in lexers.iter().filter(|l| char_filter_match(l.char_filter(), trigger_char)) {
								match lx.lex(&text[pos as usize..], &ctx).await {
									Err(e) if e.any(|e| *e == err_na) => continue,
									Err(e) => {
										let eopt = e.keep_only(|e| *e != err_cascade).map(|e| Err(e.to_api()));
										return hand.handle(&lex, &eopt).await;
									},
									Ok((s, expr)) => {
										let ctx = mk_ctx(sys, hand.reqnot()).await;
										let expr = expr
											.to_api(&mut |f, r| {
												clone!(ctx; async move { do_extra(f, r, ctx).await }).boxed_local()
											})
											.await;
										let pos = (text.len() - s.len()) as u32;
										return hand.handle(&lex, &Some(Ok(api::LexedExpr { pos, expr }))).await;
									},
								}
							}
							writeln!(logger, "Got notified about n/a character '{trigger_char}'");
							hand.handle(&lex, &None).await
						},
						api::HostExtReq::ParseLine(pline) => {
							let api::ParseLine { exported, comments, sys, line } = &pline;
							let mut ctx = mk_ctx(*sys, hand.reqnot()).await;
							let parsers = ctx.cted.inst().dyn_parsers();
							let comments = join_all(comments.iter().map(|c| Comment::from_api(c, &i))).await;
							let line: Vec<GenTokTree> = ttv_from_api(line, &mut ctx, &i).await;
							let snip = Snippet::new(line.first().expect("Empty line"), &line, &i);
							let (head, tail) = snip.pop_front().unwrap();
							let name = if let GenTok::Name(n) = &head.tok { n } else { panic!("No line head") };
							let parser =
								parsers.iter().find(|p| p.line_head() == **name).expect("No parser candidate");
							let o_line = match parser.parse(*exported, comments, tail) {
								Err(e) => Err(e.to_api()),
								Ok(t) => Ok(
									ttv_to_api(t, &mut |f, range| {
										clone!(ctx);
										async move {
											api::TokenTree { range, token: api::Token::Atom(f.clone().build(ctx).await) }
										}
										.boxed_local()
									})
									.await,
								),
							};
							hand.handle(&pline, &o_line).await
						},
						api::HostExtReq::AtomReq(atom_req) => {
							let atom = atom_req.get_atom();
							let atom_req = atom_req.clone();
							with_atom_record(&mk_ctx, hand.reqnot(), atom, move |nfo, ctx, id, buf| {
								async move {
									let actx = AtomCtx(buf, atom.drop, ctx.clone());
									match &atom_req {
										api::AtomReq::SerializeAtom(ser) => {
											let mut buf = enc_vec(&id).await;
											let refs_opt = nfo.serialize(actx, Pin::<&mut Vec<_>>::new(&mut buf)).await;
											hand.handle(ser, &refs_opt.map(|refs| (buf, refs))).await
										},
										api::AtomReq::AtomPrint(print @ api::AtomPrint(_)) =>
											hand.handle(print, &nfo.print(actx).await).await,
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
											let ret = nfo.call_ref(actx, *arg).await;
											hand.handle(call, &ret.api_return(ctx.clone(), &hand).await).await
										},
										api::AtomReq::FinalCall(call @ api::FinalCall(_, arg)) => {
											let ret = nfo.call(actx, *arg).await;
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
								}
								.boxed_local()
							})
							.await
						},
						api::HostExtReq::DeserAtom(deser) => {
							let api::DeserAtom(sys, buf, refs) = &deser;
							let mut read = &mut &buf[..];
							let ctx = mk_ctx(*sys, hand.reqnot()).await;
							let id = api::AtomId::decode(Pin::new(&mut read)).await;
							let inst = ctx.cted.inst();
							let nfo = atom_by_idx(inst.card(), id).expect("Deserializing atom with invalid ID");
							hand.handle(&deser, &nfo.deserialize(ctx.clone(), read, refs).await).await
						},
						orchid_api::HostExtReq::ApplyMacro(am) => {
							let tok = hand.will_handle_as(&am);
							let ApplyMacro { id, params, run_id, sys } = am;
							let sys_ctx = mk_ctx(sys, hand.reqnot()).await;
							let mut ctx =
								RuleCtx { args: ahash::HashMap::default(), run_id, sys: sys_ctx.clone() };
							for (k, v) in params {
								ctx.args.insert(
									Tok::from_api(k, &i).await,
									mtreev_from_api(&v, &i, &mut |_| panic!("No atom in macro prompt!")).await,
								);
							}
							let err_cascade = err_cascade(&i).await;
							let systems_g = systems.lock().await;
							let rule = &systems_g[&sys].rules[&id];
							match (rule.apply)(ctx).await {
								Err(e) => {
									let new_errors = e.keep_only(|e| *e != err_cascade);
									hand.handle_as(tok, &new_errors.map(|e| Err(e.to_api()))).await
								},
								Ok(t) => {
									let result = mtreev_to_api(&t, &mut |a| {
										clone!(sys_ctx; async move {
											api::MacroToken::Atom(a.clone().build(sys_ctx.clone()).await)
										}.boxed_local())
									})
									.await;
									hand.handle_as(tok, &Some(Ok(result))).await
								},
							}
						},
					}
				}
				.boxed_local()
			}
		},
	);
	*interner_cell.borrow_mut() = Some(Rc::new(Interner::new_replica(rn.clone().map())));
	while !exiting.load(Ordering::Relaxed) {
		let rcvd = recv_parent_msg().await.unwrap();
		rn.receive(&rcvd).await
	}
}
