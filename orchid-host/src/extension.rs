use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::num::NonZeroU64;
use std::rc::{Rc, Weak};

use async_std::channel::{self, Sender};
use async_std::sync::Mutex;
use derive_destructure::destructure;
use futures::FutureExt;
use futures::future::{join, join_all};
use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::HostMsgSet;
use orchid_api_traits::Request;
use orchid_base::builtin::ExtInit;
use orchid_base::clone;
use orchid_base::interner::Tok;
use orchid_base::logging::Logger;
use orchid_base::reqnot::{ReqNot, Requester as _};
use orchid_base::tree::AtomRepr;

use crate::api;
use crate::atom::AtomHand;
use crate::ctx::Ctx;
use crate::expr_store::ExprStore;
use crate::system::SystemCtor;

pub struct ReqPair<R: Request>(R, Sender<R::Response>);

/// Data held about an Extension. This is refcounted within [Extension]. It's
/// important to only ever access parts of this struct through the [Arc] because
/// the components reference each other through [Weak]s of it, and will panic if
/// upgrading fails.
#[derive(destructure)]
pub struct ExtensionData {
	ctx: Ctx,
	init: ExtInit,
	reqnot: ReqNot<api::HostMsgSet>,
	systems: Vec<SystemCtor>,
	logger: Logger,
	next_pars: RefCell<NonZeroU64>,
	exprs: ExprStore,
	lex_recur: Mutex<HashMap<api::ParsId, channel::Sender<ReqPair<api::SubLex>>>>,
	mac_recur: Mutex<HashMap<api::ParsId, channel::Sender<ReqPair<api::RunMacros>>>>,
}
impl Drop for ExtensionData {
	fn drop(&mut self) {
		let reqnot = self.reqnot.clone();
		(self.ctx.spawn)(Box::pin(async move { reqnot.notify(api::HostExtNotif::Exit).await }))
	}
}

#[derive(Clone)]
pub struct Extension(Rc<ExtensionData>);
impl Extension {
	pub fn new(init: ExtInit, logger: Logger, ctx: Ctx) -> io::Result<Self> {
		Ok(Self(Rc::new_cyclic(|weak: &Weak<ExtensionData>| ExtensionData {
			exprs: ExprStore::default(),
			ctx: ctx.clone(),
			systems: (init.systems.iter().cloned())
				.map(|decl| SystemCtor { decl, ext: WeakExtension(weak.clone()) })
				.collect(),
			logger: logger.clone(),
			init,
			next_pars: RefCell::new(NonZeroU64::new(1).unwrap()),
			lex_recur: Mutex::default(),
			mac_recur: Mutex::default(),
			reqnot: ReqNot::new(
				logger,
				clone!(weak; move |sfn, _| clone!(weak; async move {
					let data = weak.upgrade().unwrap();
					data.init.send(sfn).await
				}.boxed_local())),
				clone!(weak; move |notif, _| {
					clone!(weak; Box::pin(async move {
					let this = Extension(weak.upgrade().unwrap());
					match notif {
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Acquire(acq)) => {
							let target = this.0.exprs.get_expr(acq.1).expect("Invalid ticket");
							this.0.exprs.give_expr(target)
						}
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Release(rel)) => {
							this.assert_own_sys(rel.0).await;
							this.0.exprs.take_expr(rel.1)
						}
						api::ExtHostNotif::ExprNotif(api::ExprNotif::Move(mov)) => {
							this.assert_own_sys(mov.dec).await;
							let recp = this.ctx().system_inst(mov.inc).await.expect("invallid recipient sys id");
							let expr = this.0.exprs.get_expr(mov.expr).expect("invalid ticket");
							recp.ext().0.exprs.give_expr(expr);
							this.0.exprs.take_expr(mov.expr);
						},
						api::ExtHostNotif::Log(api::Log(str)) => this.logger().log(str),
					}
				}))}),
				{
					clone!(weak, ctx);
					move |hand, req| {
						clone!(weak, ctx);
						Box::pin(async move {
							let this = Self(weak.upgrade().unwrap());
							let i = this.ctx().i.clone();
							match req {
								api::ExtHostReq::Ping(ping) => hand.handle(&ping, &()).await,
								api::ExtHostReq::IntReq(intreq) => match intreq {
									api::IntReq::InternStr(s) => hand.handle(&s, &i.i(&*s.0).await.to_api()).await,
									api::IntReq::InternStrv(v) => {
										let tokens = join_all(v.0.iter().map(|m| i.ex(*m))).await;
										hand.handle(&v, &i.i(&tokens).await.to_api()).await
									},
									api::IntReq::ExternStr(si) =>
										hand.handle(&si, &Tok::<String>::from_api(si.0, &i).await.rc()).await,
									api::IntReq::ExternStrv(vi) => {
										let markerv = (i.ex(vi.0).await.iter()).map(|t| t.to_api()).collect_vec();
										hand.handle(&vi, &markerv).await
									},
								},
								api::ExtHostReq::Fwd(ref fw @ api::Fwd(ref atom, ref key, ref body)) => {
									let sys = ctx.system_inst(atom.owner).await.expect("owner of live atom dropped");
									let reply =
										sys.reqnot().request(api::Fwded(fw.0.clone(), *key, body.clone())).await;
									hand.handle(fw, &reply).await
								},
								api::ExtHostReq::SysFwd(ref fw @ api::SysFwd(id, ref body)) => {
									let sys = ctx.system_inst(id).await.unwrap();
									hand.handle(fw, &sys.request(body.clone()).await).await
								},
								api::ExtHostReq::SubLex(sl) => {
									let (rep_in, rep_out) = channel::bounded(1);
									{
										let lex_g = this.0.lex_recur.lock().await;
										let req_in = lex_g.get(&sl.id).expect("Sublex for nonexistent lexid");
										req_in.send(ReqPair(sl.clone(), rep_in)).await.unwrap();
									}
									hand.handle(&sl, &rep_out.recv().await.unwrap()).await
								},
								api::ExtHostReq::ExprReq(api::ExprReq::Inspect(ins @ api::Inspect { target })) => {
									let expr = this.exprs().get_expr(target).expect("Invalid ticket");
									hand
										.handle(&ins, &api::Inspected {
											refcount: expr.strong_count() as u32,
											location: expr.pos().to_api(),
											kind: expr.to_api().await,
										})
										.await
								},
								api::ExtHostReq::RunMacros(rm) => {
									let (rep_in, rep_out) = channel::bounded(1);
									let lex_g = this.0.mac_recur.lock().await;
									let req_in = lex_g.get(&rm.run_id).expect("Sublex for nonexistent lexid");
									req_in.send(ReqPair(rm.clone(), rep_in)).await.unwrap();
									hand.handle(&rm, &rep_out.recv().await.unwrap()).await
								},
								api::ExtHostReq::ExtAtomPrint(ref eap @ api::ExtAtomPrint(ref atom)) =>
									hand.handle(eap, &AtomHand::new(atom.clone(), &ctx).await.print().await).await,
							}
						})
					}
				},
			),
		})))
	}
	pub(crate) fn reqnot(&self) -> &ReqNot<HostMsgSet> { &self.0.reqnot }
	pub fn ctx(&self) -> &Ctx { &self.0.ctx }
	pub fn logger(&self) -> &Logger { &self.0.logger }
	pub fn system_ctors(&self) -> impl Iterator<Item = &SystemCtor> { self.0.systems.iter() }
	pub fn exprs(&self) -> &ExprStore { &self.0.exprs }
	pub async fn is_own_sys(&self, id: api::SysId) -> bool {
		let sys = self.ctx().system_inst(id).await.expect("invalid sender sys id");
		Rc::ptr_eq(&self.0, &sys.ext().0)
	}
	pub async fn assert_own_sys(&self, id: api::SysId) {
		assert!(self.is_own_sys(id).await, "Incoming message impersonates separate system");
	}
	pub fn next_pars(&self) -> NonZeroU64 {
		let mut next_pars = self.0.next_pars.borrow_mut();
		*next_pars = next_pars.checked_add(1).unwrap_or(NonZeroU64::new(1).unwrap());
		*next_pars
	}
	pub(crate) async fn lex_req<F: Future<Output = Option<api::SubLexed>>>(
		&self,
		source: Tok<String>,
		pos: u32,
		sys: api::SysId,
		mut r: impl FnMut(u32) -> F,
	) -> api::OrcResult<Option<api::LexedExpr>> {
		// get unique lex ID
		let id = api::ParsId(self.next_pars());
		// create and register channel
		let (req_in, req_out) = channel::bounded(1);
		self.0.lex_recur.lock().await.insert(id, req_in); // lex_recur released
		let (ret, ()) = join(
			async {
				let res =
					(self.reqnot()).request(api::LexExpr { id, pos, sys, text: source.to_api() }).await;
				// collect sender to unblock recursion handler branch before returning
				self.0.lex_recur.lock().await.remove(&id);
				res
			},
			async {
				while let Ok(ReqPair(sublex, rep_in)) = req_out.recv().await {
					(rep_in.send(r(sublex.pos).await).await)
						.expect("Response channel dropped while request pending")
				}
			},
		)
		.await;
		ret.transpose()
	}
	pub async fn recv_one(&self) {
		let reqnot = self.0.reqnot.clone();
		(self.0.init.recv(Box::new(move |msg| async move { reqnot.receive(msg).await }.boxed_local())))
			.await;
	}
	pub fn system_drop(&self, id: api::SysId) {
		let rc = self.clone();
		(self.ctx().spawn)(Box::pin(async move {
			rc.reqnot().notify(api::SystemDrop(id)).await;
			rc.ctx().systems.write().await.remove(&id);
		}))
	}
	pub fn downgrade(&self) -> WeakExtension { WeakExtension(Rc::downgrade(&self.0)) }
}

pub struct WeakExtension(Weak<ExtensionData>);
impl WeakExtension {
	pub fn upgrade(&self) -> Option<Extension> { self.0.upgrade().map(Extension) }
}
