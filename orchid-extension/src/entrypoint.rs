use std::io::Write;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, process, thread};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::atom::{
  Atom, AtomDrop, AtomPrint, AtomReq, AtomSame, CallRef, Command, FinalCall, Fwded, NextStep
};
use orchid_api::interner::Sweep;
use orchid_api::parser::{CharFilter, LexExpr, LexedExpr, ParserReq};
use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader, Ping};
use orchid_api::system::{SysDeclId, SysId, SystemDrop, SystemInst};
use orchid_api::tree::{GetMember, TreeId};
use orchid_api::vfs::{EagerVfs, GetVfs, VfsId, VfsRead, VfsReq};
use orchid_api_traits::{Decode, Encode};
use orchid_base::char_filter::{char_filter_match, char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::interner::{deintern, init_replica, sweep_replica};
use orchid_base::logging::Logger;
use orchid_base::name::PathSlice;
use orchid_base::reqnot::{ReqNot, Requester};

use crate::atom::{AtomCtx, AtomDynfo};
use crate::error::errv_to_apiv;
use crate::fs::VirtFS;
use crate::lexer::{CascadingError, LexContext, NotApplicableLexerError};
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::{atom_by_idx, resolv_atom, SysCtx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::{LazyMemberFactory, TIACtxImpl};

pub struct ExtensionData {
  pub thread_name: &'static str,
  pub systems: &'static [&'static dyn DynSystemCtor],
}
impl ExtensionData {
  pub fn new(thread_name: &'static str, systems: &'static [&'static dyn DynSystemCtor]) -> Self {
    Self { thread_name, systems }
  }
  pub fn main(self) {
    extension_main(self)
  }
}

pub enum MemberRecord {
  Gen(LazyMemberFactory),
  Res,
}

pub struct SystemRecord {
  cted: CtedObj,
  vfses: HashMap<VfsId, &'static dyn VirtFS>,
  declfs: EagerVfs,
  lazy_members: HashMap<TreeId, MemberRecord>,
}

pub fn with_atom_record<T>(
  systems: &Mutex<HashMap<SysId, SystemRecord>>,
  atom: &Atom,
  cb: impl FnOnce(&'static dyn AtomDynfo, CtedObj, &[u8]) -> T,
) -> T {
  let mut data = &atom.data[..];
  let systems_g = systems.lock().unwrap();
  let cted = &systems_g[&atom.owner].cted;
  let sys = cted.inst();
  let atom_record = atom_by_idx(sys.dyn_card(), u64::decode(&mut data)).expect("Atom ID reserved");
  cb(atom_record, cted.clone(), data)
}

pub fn extension_main(data: ExtensionData) {
  if thread::Builder::new().name(data.thread_name.to_string()).spawn(|| extension_main_logic(data)).unwrap().join().is_err() {
    process::exit(-1)
  }
}

fn extension_main_logic(data: ExtensionData) {
  let HostHeader{ log_strategy } = HostHeader::decode(&mut std::io::stdin().lock());
  let mut buf = Vec::new();
  let decls = (data.systems.iter().enumerate())
    .map(|(id, sys)| (u16::try_from(id).expect("more than u16max system ctors"), sys))
    .map(|(id, sys)| sys.decl(SysDeclId(NonZero::new(id + 1).unwrap())))
    .collect_vec();
  let systems = Arc::new(Mutex::new(HashMap::<SysId, SystemRecord>::new()));
  ExtensionHeader { systems: decls.clone() }.encode(&mut buf);
  std::io::stdout().write_all(&buf).unwrap();
  std::io::stdout().flush().unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let logger = Arc::new(Logger::new(log_strategy));
  let rn = ReqNot::<ExtMsgSet>::new(
    |a, _| {
      eprintln!("Upsending {:?}", a);
      send_parent_msg(a).unwrap()
    },
    clone!(systems, exiting, logger; move |n, reqnot| match n {
      HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      HostExtNotif::SystemDrop(SystemDrop(sys_id)) =>
        mem::drop(systems.lock().unwrap().remove(&sys_id)),
      HostExtNotif::AtomDrop(AtomDrop(atom)) => {
        with_atom_record(&systems, &atom, |rec, cted, data| {
          rec.drop(AtomCtx(data, SysCtx{ reqnot, logger: logger.clone(), id: atom.owner, cted }))
        })
      }
    }),
    clone!(systems, logger; move |req| match req.req() {
      HostExtReq::Ping(ping@Ping) => req.handle(ping, &()),
      HostExtReq::Sweep(sweep@Sweep) => req.handle(sweep, &sweep_replica()),
      HostExtReq::NewSystem(new_sys) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let cted = data.systems[i].new_system(new_sys);
        let mut vfses = HashMap::new();
        let lex_filter = cted.inst().dyn_lexers().iter().fold(CharFilter(vec![]), |cf, lx| {
          let lxcf = mk_char_filter(lx.char_filter().iter().cloned());
          char_filter_union(&cf, &lxcf)
        });
        let mut lazy_mems = HashMap::new();
        let const_root = (cted.inst().dyn_env().into_iter())
          .map(|(k, v)| {
            (k.marker(), v.into_api(&mut TIACtxImpl{ lazy: &mut lazy_mems, sys: &*cted.inst()}))
          })
          .collect();
        systems.lock().unwrap().insert(new_sys.id, SystemRecord {
          declfs: cted.inst().dyn_vfs().to_api_rec(&mut vfses),
          vfses,
          cted,
          lazy_members: lazy_mems
        });
        req.handle(new_sys, &SystemInst {
          lex_filter,
          const_root,
          parses_lines: vec!()
        })
      }
      HostExtReq::GetMember(get_tree@GetMember(sys_id, tree_id)) => {
        let mut systems_g = systems.lock().unwrap();
        let sys = systems_g.get_mut(sys_id).expect("System not found");
        let lazy = &mut sys.lazy_members;
        let cb = match lazy.insert(*tree_id, MemberRecord::Res) {
          None => panic!("Tree for ID not found"),
          Some(MemberRecord::Res) => panic!("This tree has already been transmitted"),
          Some(MemberRecord::Gen(cb)) => cb,
        };
        let tree = cb.build();
        let reply_tree = tree.into_api(&mut TIACtxImpl{ sys: &*sys.cted.inst(), lazy });
        req.handle(get_tree, &reply_tree);
      }
      HostExtReq::VfsReq(VfsReq::GetVfs(get_vfs@GetVfs(sys_id))) => {
        let systems_g = systems.lock().unwrap();
        req.handle(get_vfs, &systems_g[sys_id].declfs)
      }
      HostExtReq::VfsReq(VfsReq::VfsRead(vfs_read@VfsRead(sys_id, vfs_id, path))) => {
        let systems_g = systems.lock().unwrap();
        let path = path.iter().map(|t| deintern(*t)).collect_vec();
        req.handle(vfs_read, &systems_g[sys_id].vfses[vfs_id].load(PathSlice::new(&path)))
      }
      HostExtReq::ParserReq(ParserReq::LexExpr(lex)) => {
        let LexExpr{ sys, text, pos, id } = *lex;
        let systems_g = systems.lock().unwrap();
        let lexers = systems_g[&sys].cted.inst().dyn_lexers();
        mem::drop(systems_g);
        let text = deintern(text);
        let tk = req.will_handle_as(lex);
        thread::spawn(clone!(systems; move || {
          let ctx = LexContext { sys, id, pos, reqnot: req.reqnot(), text: &text };
          let trigger_char = text.chars().nth(pos as usize).unwrap();
          for lx in lexers.iter().filter(|l| char_filter_match(l.char_filter(), trigger_char)) {
            match lx.lex(&text[pos as usize..], &ctx) {
              Err(e) if e.as_any_ref().is::<NotApplicableLexerError>() => continue,
              Err(e) if e.as_any_ref().is::<CascadingError>() => return req.handle_as(tk, &None),
              Err(e) => return req.handle_as(tk, &Some(Err(errv_to_apiv([e])))),
              Ok((s, expr)) => {
                let systems_g = systems.lock().unwrap();
                let expr = expr.into_api(&*systems_g[&sys].cted.inst());
                let pos = (text.len() - s.len()) as u32;
                return req.handle_as(tk, &Some(Ok(LexedExpr{ pos, expr })))
              }
            }
          }
          eprintln!("Got notified about n/a character '{trigger_char}'");
          req.handle_as(tk, &None)
        }));
      },
      HostExtReq::AtomReq(atom_req) => {
        let systems_g = systems.lock().unwrap();
        let atom = atom_req.get_atom();
        let sys = &systems_g[&atom.owner];
        let ctx = SysCtx {
          cted: sys.cted.clone(),
          id: atom.owner,
          logger: logger.clone(),
          reqnot: req.reqnot()
        };
        let dynfo = resolv_atom(&*sys.cted.inst(), atom);
        let actx = AtomCtx(&atom.data[8..], ctx);
        match atom_req {
          AtomReq::AtomPrint(print@AtomPrint(_)) => req.handle(print, &dynfo.print(actx)),
          AtomReq::AtomSame(same@AtomSame(_, r)) => {
            // different systems or different type tags
            if atom.owner != r.owner || atom.data[..8] != r.data[..8] {
              return req.handle(same, &false)
            }
            req.handle(same, &dynfo.same(actx, &r.data[8..]))
          },
          AtomReq::Fwded(fwded@Fwded(_, payload)) => {
            let mut reply = Vec::new();
            dynfo.handle_req(actx, &mut &payload[..], &mut reply);
            req.handle(fwded, &reply)
          }
          AtomReq::CallRef(call@CallRef(_, arg))
            => req.handle(call, &dynfo.call_ref(actx, *arg).to_api(&*sys.cted.inst())),
          AtomReq::FinalCall(call@FinalCall(_, arg))
            => req.handle(call, &dynfo.call(actx, *arg).to_api(&*sys.cted.inst())),
          AtomReq::Command(cmd@Command(_)) => req.handle(cmd, &match dynfo.command(actx) {
            Err(e) => Err(errv_to_apiv([e])),
            Ok(opt) => Ok(match opt {
              Some(cont) => NextStep::Continue(cont.into_api(&*sys.cted.inst())),
              None => NextStep::Halt,
            })
          })
        }
      }
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    let rcvd = recv_parent_msg().unwrap();
    // eprintln!("Downsent {rcvd:?}");
    rn.receive(rcvd)
  }
}
