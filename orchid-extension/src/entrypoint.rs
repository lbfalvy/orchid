use std::num::{NonZeroU16, NonZeroU64};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, thread};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::atom::{Atom, AtomDrop, AtomReq, AtomSame, CallRef, Command, FinalCall, Fwded};
use orchid_api::interner::Sweep;
use orchid_api::parser::{CharFilter, Lex, Lexed, ParserReq};
use orchid_api::proto::{ExtMsgSet, ExtensionHeader, HostExtNotif, HostExtReq, HostHeader, Ping};
use orchid_api::system::{SysId, SystemDrop, SystemInst};
use orchid_api::tree::{GetConstTree, Tree, TreeId};
use orchid_api::vfs::{EagerVfs, GetVfs, VfsId, VfsRead, VfsReq};
use orchid_api_traits::{Decode, Encode};
use orchid_base::char_filter::{char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::interner::{deintern, init_replica, sweep_replica};
use orchid_base::name::PathSlice;
use orchid_base::reqnot::{ReqNot, Requester};

use crate::atom::AtomInfo;
use crate::error::{err_or_ref_to_api, unpack_err};
use crate::fs::VirtFS;
use crate::lexer::LexContext;
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::{atom_by_idx, SysCtx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::LazyTreeFactory;

pub struct ExtensionData {
  pub systems: &'static [&'static dyn DynSystemCtor],
}

pub enum TreeRecord {
  Gen(LazyTreeFactory),
  Res(Tree),
}

pub struct SystemRecord {
  cted: CtedObj,
  vfses: HashMap<VfsId, &'static dyn VirtFS>,
  declfs: EagerVfs,
  tree: Tree,
  subtrees: HashMap<TreeId, TreeRecord>,
}

pub fn with_atom_record<T>(
  systems: &Mutex<HashMap<SysId, SystemRecord>>,
  atom: &Atom,
  cb: impl FnOnce(&AtomInfo, CtedObj, &[u8]) -> T,
) -> T {
  let mut data = &atom.data[..];
  let systems_g = systems.lock().unwrap();
  let cted = &systems_g[&atom.owner].cted;
  let sys = cted.inst();
  let atom_record = atom_by_idx(sys.dyn_card(), u64::decode(&mut data)).expect("Atom ID reserved");
  cb(atom_record, cted.clone(), data)
}

pub fn extension_main(data: ExtensionData) {
  HostHeader::decode(&mut &recv_parent_msg().unwrap()[..]);
  let mut buf = Vec::new();
  let decls = (data.systems.iter().enumerate())
    .map(|(id, sys)| (u16::try_from(id).expect("more than u16max system ctors"), sys))
    .map(|(id, sys)| sys.decl(NonZeroU16::new(id + 1).unwrap()))
    .collect_vec();
  let systems = Arc::new(Mutex::new(HashMap::<SysId, SystemRecord>::new()));
  ExtensionHeader { systems: decls.clone() }.encode(&mut buf);
  send_parent_msg(&buf).unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let rn = ReqNot::<ExtMsgSet>::new(
    |a, _| send_parent_msg(a).unwrap(),
    clone!(systems, exiting; move |n, reqnot| match n {
      HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      HostExtNotif::SystemDrop(SystemDrop(sys_id)) =>
        mem::drop(systems.lock().unwrap().remove(&sys_id)),
      HostExtNotif::AtomDrop(AtomDrop(atom)) => {
        with_atom_record(&systems, &atom, |rec, cted, data| {
          (rec.drop)(data, SysCtx{ reqnot, id: atom.owner, cted })
        })
      }
    }),
    clone!(systems; move |req| match req.req() {
      HostExtReq::Ping(ping@Ping) => req.handle(ping, &()),
      HostExtReq::Sweep(sweep@Sweep) => req.handle(sweep, &sweep_replica()),
      HostExtReq::NewSystem(new_sys) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let cted = data.systems[i].new_system(new_sys);
        let mut vfses = HashMap::new();
        let lex_filter = cted.inst().dyn_lexers().iter().fold(CharFilter(vec![]), |cf, lx| {
          char_filter_union(&cf, &mk_char_filter(lx.char_filter().iter().cloned()))
        });
        let mut subtrees = HashMap::new();
        systems.lock().unwrap().insert(new_sys.id, SystemRecord {
          declfs: cted.inst().dyn_vfs().to_api_rec(&mut vfses),
          vfses,
          tree: cted.inst().dyn_env().into_api(&*cted.inst(), &mut |gen| {
            let id = TreeId::new((subtrees.len() + 2) as u64).unwrap();
            subtrees.insert(id, TreeRecord::Gen(gen.clone()));
            id
          }),
          cted,
          subtrees
        });
        req.handle(new_sys, &SystemInst {
          lex_filter, const_root_id: NonZeroU64::new(1).unwrap()
        })
      }
      HostExtReq::GetConstTree(get_tree@GetConstTree(sys_id, tree_id)) => {
        let mut systems_g = systems.lock().unwrap();
        let sys = systems_g.get_mut(sys_id).expect("System not found");
        if tree_id.get() == 1 {
          req.handle(get_tree, &sys.tree);
        } else {
          let subtrees = &mut sys.subtrees;
          let tree_rec = subtrees.get_mut(tree_id).expect("Tree for ID not found");
          match tree_rec {
            TreeRecord::Res(tree) => req.handle(get_tree, tree),
            TreeRecord::Gen(cb) => {
              let tree = cb.build();
              let reply_tree = tree.into_api(&*sys.cted.inst(), &mut |cb| {
                let id = NonZeroU64::new((subtrees.len() + 2) as u64).unwrap();
                subtrees.insert(id, TreeRecord::Gen(cb.clone()));
                id
              });
              req.handle(get_tree, &reply_tree);
              subtrees.insert(*tree_id, TreeRecord::Res(reply_tree));
            }
          }
        }
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
      HostExtReq::ParserReq(ParserReq::Lex(lex)) => {
        let Lex{ sys, text, pos, id } = *lex;
        let systems_g = systems.lock().unwrap();
        let lexers = systems_g[&sys].cted.inst().dyn_lexers();
        mem::drop(systems_g);
        let text = deintern(text);
        let tk = req.will_handle_as(lex);
        thread::spawn(clone!(systems; move || {
          let ctx = LexContext { sys, id, pos, reqnot: req.reqnot(), text: &text };
          let lex_res = lexers.iter().find_map(|lx| lx.lex(&text[pos as usize..], &ctx));
          req.handle_as(tk, &lex_res.map(|r| match r {
            Ok((s, data)) => {
              let systems_g = systems.lock().unwrap();
              let data = data.into_api(&*systems_g[&sys].cted.inst());
              Ok(Lexed { data, pos: (text.len() - s.len()) as u32 })
            },
            Err(e) => Err(unpack_err(e).into_iter().map(err_or_ref_to_api).collect_vec())
          }))
        }));
      },
      HostExtReq::AtomReq(AtomReq::AtomSame(same@AtomSame(l, r))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::Fwded(call@Fwded(atom, req))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::CallRef(call@CallRef(atom, arg))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::FinalCall(call@FinalCall(atom, arg))) => todo!("subsys nimpl"),
      HostExtReq::AtomReq(AtomReq::Command(cmd@Command(atom))) => todo!("subsys impl"),
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    rn.receive(recv_parent_msg().unwrap())
  }
}
