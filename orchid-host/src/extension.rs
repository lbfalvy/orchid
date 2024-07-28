use std::io::Write as _;
use std::num::NonZero;
use std::ops::Deref;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::{fmt, io, process, thread};

use derive_destructure::destructure;
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use itertools::Itertools;
use lazy_static::lazy_static;
use orchid_api::atom::{Atom, AtomDrop, AtomSame, CallRef, FinalCall, Fwd, Fwded};
use orchid_api::error::ProjResult;
use orchid_api::expr::{Acquire, Expr, ExprNotif, ExprTicket, Release, Relocate};
use orchid_api::interner::IntReq;
use orchid_api::parser::{CharFilter, LexExpr, LexedExpr, ParsId, SubLex, SubLexed};
use orchid_api::proto::{
  ExtHostNotif, ExtHostReq, ExtensionHeader, HostExtNotif, HostHeader, HostMsgSet,
};
use orchid_api::system::{NewSystem, SysDeclId, SysId, SystemDecl, SystemDrop};
use orchid_api::tree::{GetMember, Member, MemberKind, TreeId};
use orchid_api_traits::{Decode, Encode, Request};
use orchid_base::char_filter::char_filter_match;
use orchid_base::clone;
use orchid_base::interner::{deintern, intern, Tok};
use orchid_base::reqnot::{ReqNot, Requester as _};
use ordered_float::NotNan;

use crate::expr::RtExpr;
use crate::tree::OwnedMember;

#[derive(Debug, destructure)]
pub struct AtomData {
  owner: System,
  drop: bool,
  data: Vec<u8>,
}
impl AtomData {
  fn api(self) -> Atom {
    let (owner, drop, data) = self.destructure();
    Atom { data, drop, owner: owner.id() }
  }
  fn api_ref(&self) -> Atom {
    Atom { data: self.data.clone(), drop: self.drop, owner: self.owner.id() }
  }
}
impl Drop for AtomData {
  fn drop(&mut self) {
    self.owner.reqnot().notify(AtomDrop(Atom {
      owner: self.owner.id(),
      data: self.data.clone(),
      drop: true,
    }))
  }
}

#[derive(Clone, Debug)]
pub struct AtomHand(Arc<AtomData>);
impl AtomHand {
  pub fn from_api(Atom { data, drop, owner }: Atom) -> Self {
    let owner = System::resolve(owner).expect("Atom owned by non-existing system");
    Self(Arc::new(AtomData { data, drop, owner }))
  }
  pub fn call(self, arg: RtExpr) -> Expr {
    let owner_sys = self.0.owner.clone();
    let reqnot = owner_sys.reqnot();
    let ticket = owner_sys.give_expr(arg.canonicalize(), || arg);
    match Arc::try_unwrap(self.0) {
      Ok(data) => reqnot.request(FinalCall(data.api(), ticket)),
      Err(hand) => reqnot.request(CallRef(hand.api_ref(), ticket)),
    }
  }
  pub fn same(&self, other: &AtomHand) -> bool {
    let owner = self.0.owner.id();
    if other.0.owner.id() != owner {
      return false;
    }
    self.0.owner.reqnot().request(AtomSame(self.0.api_ref(), other.0.api_ref()))
  }
  pub fn req(&self, req: Vec<u8>) -> Vec<u8> {
    self.0.owner.reqnot().request(Fwded(self.0.api_ref(), req))
  }
  pub fn api_ref(&self) -> Atom { self.0.api_ref() }
}

/// Data held about an Extension. This is refcounted within [Extension]. It's
/// important to only ever access parts of this struct through the [Arc] because
/// the components reference each other through [Weak]s of it, and will panic if
/// upgrading fails.
#[derive(destructure)]
pub struct ExtensionData {
  child: Mutex<process::Child>,
  reqnot: ReqNot<HostMsgSet>,
  systems: Vec<SystemCtor>,
}
impl Drop for ExtensionData {
  fn drop(&mut self) { self.reqnot.notify(HostExtNotif::Exit) }
}

fn acq_expr(sys: SysId, extk: ExprTicket) {
  (System::resolve(sys).expect("Expr acq'd by invalid system"))
    .give_expr(extk, || RtExpr::resolve(extk).expect("Invalid expr acq'd"));
}

fn rel_expr(sys: SysId, extk: ExprTicket) {
  let sys = System::resolve(sys).unwrap();
  let mut exprs = sys.0.exprs.write().unwrap();
  exprs.entry(extk).and_replace_entry_with(|_, (rc, rt)| {
    (0 < rc.fetch_sub(1, Ordering::Relaxed)).then_some((rc, rt))
  });
}

#[derive(Clone)]
pub struct Extension(Arc<ExtensionData>);
impl Extension {
  pub fn new(mut cmd: process::Command) -> io::Result<Self> {
    let mut child = cmd.stdin(process::Stdio::piped()).stdout(process::Stdio::piped()).spawn()?;
    HostHeader.encode(child.stdin.as_mut().unwrap());
    let eh = ExtensionHeader::decode(child.stdout.as_mut().unwrap());
    Ok(Self(Arc::new_cyclic(|weak| ExtensionData {
      child: Mutex::new(child),
      reqnot: ReqNot::new(
        clone!(weak; move |sfn, _| {
          let arc: Arc<ExtensionData> = weak.upgrade().unwrap();
          let mut g = arc.child.lock().unwrap();
          g.stdin.as_mut().unwrap().write_all(sfn).unwrap();
        }),
        |notif, _| match notif {
          ExtHostNotif::ExprNotif(ExprNotif::Acquire(Acquire(sys, extk))) => acq_expr(sys, extk),
          ExtHostNotif::ExprNotif(ExprNotif::Release(Release(sys, extk))) => rel_expr(sys, extk),
          ExtHostNotif::ExprNotif(ExprNotif::Relocate(Relocate { dec, inc, expr })) => {
            acq_expr(inc, expr);
            rel_expr(dec, expr);
          },
          ExtHostNotif::AdviseSweep(_advice) => eprintln!("Sweep advice is unsupported")
        },
        |req| match req.req() {
          ExtHostReq::Ping(ping) => req.handle(ping, &()),
          ExtHostReq::IntReq(IntReq::InternStr(s)) => req.handle(s, &intern(&**s.0).marker()),
          ExtHostReq::IntReq(IntReq::InternStrv(v)) => req.handle(v, &intern(&*v.0).marker()),
          ExtHostReq::IntReq(IntReq::ExternStr(si)) => req.handle(si, &deintern(si.0).arc()),
          ExtHostReq::IntReq(IntReq::ExternStrv(vi)) =>
            req.handle(vi, &Arc::new(deintern(vi.0).iter().map(|t| t.marker()).collect_vec())),
          ExtHostReq::Fwd(fw @ Fwd(atom, _body)) => {
            let sys = System::resolve(atom.owner).unwrap();
            thread::spawn(clone!(fw; move || {
              req.handle(&fw, &sys.reqnot().request(Fwded(fw.0.clone(), fw.1.clone())))
            }));
          },
          ExtHostReq::SubLex(sl) => {
            let lex_g = LEX_RECUR.lock().unwrap();
            let (rep_in, rep_out) = sync_channel(0);
            let req_in = lex_g.get(&sl.id).expect("Sublex for nonexistent lexid");
            req_in.send(ReqPair(sl.clone(), rep_in)).unwrap();
            req.handle(sl, &rep_out.recv().unwrap())
          },
          _ => todo!(),
        },
      ),
      systems: eh.systems.into_iter().map(|decl| SystemCtor { decl, ext: weak.clone() }).collect(),
    })))
  }
  pub fn systems(&self) -> impl Iterator<Item = &SystemCtor> { self.0.systems.iter() }
}

pub struct SystemCtor {
  decl: SystemDecl,
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
    let id = SysId(NonZero::new(NEXT_ID.fetch_add(1, Ordering::Relaxed)).expect("next_id wrapped"));
    let sys_inst = ext.reqnot.request(NewSystem { depends, id, system: self.decl.id });
    let data = System(Arc::new(SystemInstData {
      decl_id: self.decl.id,
      ext: Extension(ext),
      exprs: RwLock::default(),
      lex_filter: sys_inst.lex_filter,
      const_root: OnceLock::new(),
      id,
    }));
    let root = (sys_inst.const_root.into_iter())
        .map(|(k, v)| OwnedMember::from_api(Member { public: true, name: k, kind: v }, &data))
        .collect_vec();
      data.0.const_root.set(root).unwrap();
    inst_g.insert(id, data.clone());
    data
  }
}

lazy_static! {
  static ref SYSTEM_INSTS: RwLock<HashMap<SysId, System>> = RwLock::default();
  static ref LEX_RECUR: Mutex<HashMap<ParsId, SyncSender<ReqPair<SubLex>>>> = Mutex::default();
}

pub struct ReqPair<R: Request>(R, pub SyncSender<R::Response>);

#[derive(destructure)]
pub struct SystemInstData {
  exprs: RwLock<HashMap<ExprTicket, (AtomicU32, RtExpr)>>,
  ext: Extension,
  decl_id: SysDeclId,
  lex_filter: CharFilter,
  id: SysId,
  const_root: OnceLock<Vec<OwnedMember>>,
}
impl Drop for SystemInstData {
  fn drop(&mut self) {
    self.ext.0.reqnot.notify(SystemDrop(self.id));
    if let Ok(mut g) = SYSTEM_INSTS.write() {
      g.remove(&self.id);
    }
  }
}
#[derive(Clone)]
pub struct System(Arc<SystemInstData>);
impl System {
  pub fn id(&self) -> SysId { self.id }
  fn resolve(id: SysId) -> Option<System> { SYSTEM_INSTS.read().unwrap().get(&id).cloned() }
  fn reqnot(&self) -> &ReqNot<HostMsgSet> { &self.0.ext.0.reqnot }
  fn give_expr(&self, ticket: ExprTicket, get_expr: impl FnOnce() -> RtExpr) -> ExprTicket {
    match self.0.exprs.write().unwrap().entry(ticket) {
      Entry::Occupied(mut oe) => {
        oe.get_mut().0.fetch_add(1, Ordering::Relaxed);
      },
      Entry::Vacant(v) => {
        v.insert((AtomicU32::new(1), get_expr()));
      }
    }
    ticket
  }
  pub fn get_tree(&self, id: TreeId) -> MemberKind {
    self.reqnot().request(GetMember(self.0.id, id))
  }
  pub fn has_lexer(&self) -> bool { !self.0.lex_filter.0.is_empty() }
  pub fn can_lex(&self, c: char) -> bool { char_filter_match(&self.0.lex_filter, c) }
  /// Have this system lex a part of the source. It is assumed that
  /// [Self::can_lex] was called and returned true.
  pub fn lex(
    &self,
    source: Tok<String>,
    pos: u32,
    mut r: impl FnMut(u32) -> Option<SubLexed> + Send,
  ) -> ProjResult<Option<LexedExpr>> {
    // get unique lex ID
    static LEX_ID: AtomicU64 = AtomicU64::new(1);
    let id = ParsId(NonZero::new(LEX_ID.fetch_add(1, Ordering::Relaxed)).unwrap());
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
      let ret = self.reqnot().request(LexExpr { id, pos, sys: self.id(), text: source.marker() });
      // collect sender to unblock recursion handler thread before returning
      LEX_RECUR.lock().unwrap().remove(&id);
      ret.transpose()
    }) // exit recursion handler thread
  }
}
impl fmt::Debug for System {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let ctor = (self.0.ext.0.systems.iter().find(|c| c.decl.id == self.0.decl_id))
      .expect("System instance with no associated constructor");
    write!(f, "System({} @ {} #{}, ", ctor.decl.name, ctor.decl.priority, self.0.id.0)?;
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
