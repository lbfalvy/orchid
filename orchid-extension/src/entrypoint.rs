use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, thread};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::atom::{Atom, AtomReq, AtomSame, CallRef, FinalCall, Fwded};
use orchid_api::parser::{CharFilter, Lex, Lexed, ParserReq, SubLex};
use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader};
use orchid_api::system::{SysId, SystemInst};
use orchid_api::vfs::{EagerVfs, VfsId, VfsRead, VfsReq};
use orchid_api_traits::{Decode, Encode};
use orchid_base::char_filter::{char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::intern::{deintern, init_replica, sweep_replica};
use orchid_base::name::PathSlice;
use orchid_base::reqnot::{ReqNot, Requester};

use crate::atom::AtomInfo;
use crate::fs::VirtFS;
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::DynSystem;
use crate::system_ctor::DynSystemCtor;

pub struct ExtensionData {
  pub systems: Vec<Box<dyn DynSystemCtor>>,
}

pub struct SystemRecord {
  instance: Box<dyn DynSystem>,
  vfses: HashMap<VfsId, Arc<dyn VirtFS>>,
  declfs: EagerVfs,
}

pub fn with_atom_record<T>(
  systems: &Mutex<HashMap<SysId, SystemRecord>>,
  atom: &Atom,
  cb: impl FnOnce(&AtomInfo, &[u8]) -> T,
) -> T {
  let mut data = &atom.data[..];
  let systems_g = systems.lock().unwrap();
  let sys = &systems_g[&atom.owner].instance;
  let atom_record =
    (sys.card().atoms()[u64::decode(&mut data) as usize].as_ref()).expect("Atom ID reserved");
  cb(atom_record, data)
}

pub fn main(data: ExtensionData) {
  HostHeader::decode(&mut &recv_parent_msg().unwrap()[..]);
  let mut buf = Vec::new();
  let decls = data.systems.iter().map(|sys| sys.decl()).collect_vec();
  let systems = Arc::new(Mutex::new(HashMap::<SysId, SystemRecord>::new()));
  ExtensionHeader { systems: decls.clone() }.encode(&mut buf);
  send_parent_msg(&buf).unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let rn = ReqNot::<ExtMsgSet>::new(
    |a, _| send_parent_msg(a).unwrap(),
    clone!(systems, exiting; move |n, _| match n {
      HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      HostExtNotif::SystemDrop(sys) => mem::drop(systems.lock().unwrap().remove(&sys.0)),
      HostExtNotif::AtomDrop(atom) =>
        with_atom_record(&systems, &atom.0, |rec, data| (rec.drop)(data)),
    }),
    clone!(systems; move |req| match req.req() {
      HostExtReq::Ping(ping) => req.handle(ping, &()),
      HostExtReq::Sweep(sweep) => req.handle(sweep, &sweep_replica()),
      HostExtReq::NewSystem(new_sys) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let system = data.systems[i].new_system(new_sys, req.reqnot());
        let mut vfses = HashMap::new();
        let lex_filter = system.lexers().iter().fold(CharFilter(vec![]), |cf, lx| {
          char_filter_union(&cf, &mk_char_filter(lx.char_filter().iter().cloned()))
        });
        systems.lock().unwrap().insert(new_sys.id, SystemRecord {
          declfs: system.source().to_api_rec(&mut vfses),
          vfses,
          instance: system,
        });
        req.handle(new_sys, &SystemInst {
          lex_filter
        })
      }
      HostExtReq::GetConstTree(get_tree) => {
        let systems_g = systems.lock().unwrap();
        req.handle(get_tree, &systems_g[&get_tree.0].instance.env())
      }
      HostExtReq::VfsReq(VfsReq::GetVfs(get_vfs)) => {
        let systems_g = systems.lock().unwrap();
        req.handle(get_vfs, &systems_g[&get_vfs.0].declfs)
      }
      HostExtReq::VfsReq(VfsReq::VfsRead(vfs_read@VfsRead(sys_id, vfs_id, path))) => {
        let systems_g = systems.lock().unwrap();
        let path = path.iter().map(|t| deintern(*t)).collect_vec();
        req.handle(vfs_read, &systems_g[sys_id].vfses[vfs_id].load(PathSlice::new(&path)))
      }
      HostExtReq::ParserReq(ParserReq::Lex(lex)) => {
        let systems_g = systems.lock().unwrap();
        let Lex{ sys, text, pos } = *lex;
        let lexers = systems_g[&sys].instance.lexers();
        mem::drop(systems_g);
        let source = deintern(text);
        let tk = req.will_handle_as(lex);
        thread::spawn(move || {
          let reqnot = req.reqnot();
          let mut recurse = |tail: &str| {
            let pos = (source.len() - tail.len()) as u32;
            let lexed = reqnot.request(SubLex{ pos, text })?;
            Ok((&source[lexed.pos as usize..], lexed.data))
          };
          let lex_res = lexers.iter().find_map(|lx| lx.lex(&source[pos as usize..], &mut recurse));
          req.handle_as(tk, &lex_res.map(|r| r.map(|(s, data)| {
            let pos = (source.len() - s.len()) as u32;
            Lexed { data, pos }
          })))
        });
      },
      HostExtReq::AtomReq(AtomReq::AtomSame(same@AtomSame(l, r))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::Fwded(call@Fwded(atom, req))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::CallRef(call@CallRef(atom, arg))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::FinalCall(call@FinalCall(atom, arg))) => todo!("subsys nimpl"),
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    rn.receive(recv_parent_msg().unwrap())
  }
}
