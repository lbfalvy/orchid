use std::io::Write;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, process, thread};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api::DeserAtom;
use orchid_api_traits::{enc_vec, Decode, Encode};
use orchid_base::char_filter::{char_filter_match, char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::error::errv_to_apiv;
use orchid_base::interner::{deintern, init_replica, sweep_replica};
use orchid_base::logging::Logger;
use orchid_base::name::{PathSlice, Sym};
use orchid_base::parse::Snippet;
use orchid_base::reqnot::{ReqNot, Requester};
use orchid_base::tree::{ttv_from_api, ttv_to_api};
use substack::Substack;

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo};
use crate::atom_owned::OBJ_STORE;
use crate::fs::VirtFS;
use crate::lexer::{err_cascade, err_lexer_na, LexContext};
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::{atom_by_idx, SysCtx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::{do_extra, GenTok, GenTokTree, LazyMemberFactory, TIACtxImpl};

pub struct ExtensionData {
  pub name: &'static str,
  pub systems: &'static [&'static dyn DynSystemCtor],
}
impl ExtensionData {
  pub fn new(name: &'static str, systems: &'static [&'static dyn DynSystemCtor]) -> Self {
    Self { name, systems }
  }
  pub fn main(self) { extension_main(self) }
}

pub enum MemberRecord {
  Gen(Sym, LazyMemberFactory),
  Res,
}

pub struct SystemRecord {
  cted: CtedObj,
  vfses: HashMap<api::VfsId, &'static dyn VirtFS>,
  declfs: api::EagerVfs,
  lazy_members: HashMap<api::TreeId, MemberRecord>,
}

pub fn with_atom_record<T>(
  get_sys_ctx: &impl Fn(api::SysId, ReqNot<api::ExtMsgSet>) -> SysCtx,
  reqnot: ReqNot<api::ExtMsgSet>,
  atom: &api::Atom,
  cb: impl FnOnce(&'static dyn AtomDynfo, SysCtx, api::AtomId, &[u8]) -> T,
) -> T {
  let mut data = &atom.data[..];
  let ctx = get_sys_ctx(atom.owner, reqnot);
  let inst = ctx.cted.inst();
  let id = api::AtomId::decode(&mut data);
  let atom_record = atom_by_idx(inst.card(), id).expect("Atom ID reserved");
  cb(atom_record, ctx, id, data)
}

pub fn extension_main(data: ExtensionData) {
  if thread::Builder::new()
    .name(format!("ext-main:{}", data.name))
    .spawn(|| extension_main_logic(data))
    .unwrap()
    .join()
    .is_err()
  {
    process::exit(-1)
  }
}

fn extension_main_logic(data: ExtensionData) {
  let api::HostHeader { log_strategy } = api::HostHeader::decode(&mut std::io::stdin().lock());
  let mut buf = Vec::new();
  let decls = (data.systems.iter().enumerate())
    .map(|(id, sys)| (u16::try_from(id).expect("more than u16max system ctors"), sys))
    .map(|(id, sys)| sys.decl(api::SysDeclId(NonZero::new(id + 1).unwrap())))
    .collect_vec();
  let systems = Arc::new(Mutex::new(HashMap::<api::SysId, SystemRecord>::new()));
  api::ExtensionHeader { name: data.name.to_string(), systems: decls.clone() }.encode(&mut buf);
  std::io::stdout().write_all(&buf).unwrap();
  std::io::stdout().flush().unwrap();
  let exiting = Arc::new(AtomicBool::new(false));
  let logger = Arc::new(Logger::new(log_strategy));
  let mk_ctx = clone!(logger, systems; move |id: api::SysId, reqnot: ReqNot<api::ExtMsgSet>| {
    let cted = systems.lock().unwrap()[&id].cted.clone();
    SysCtx { id, cted, logger: logger.clone(), reqnot }
  });
  let rn = ReqNot::<api::ExtMsgSet>::new(
    clone!(logger; move |a, _| {
      logger.log_buf("Upsending", a);
      send_parent_msg(a).unwrap()
    }),
    clone!(systems, exiting, mk_ctx; move |n, reqnot| match n {
      api::HostExtNotif::Exit => exiting.store(true, Ordering::Relaxed),
      api::HostExtNotif::SystemDrop(api::SystemDrop(sys_id)) =>
        mem::drop(systems.lock().unwrap().remove(&sys_id)),
      api::HostExtNotif::AtomDrop(api::AtomDrop(sys_id, atom)) =>
        OBJ_STORE.get(atom.0).unwrap().remove().dyn_free(mk_ctx(sys_id, reqnot)),
    }),
    clone!(systems, logger; move |req| match req.req() {
      api::HostExtReq::Ping(ping@api::Ping) => req.handle(ping, &()),
      api::HostExtReq::Sweep(sweep@api::Sweep) => req.handle(sweep, &sweep_replica()),
      api::HostExtReq::SysReq(api::SysReq::NewSystem(new_sys)) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let cted = data.systems[i].new_system(new_sys);
        let mut vfses = HashMap::new();
        let lex_filter = cted.inst().dyn_lexers().iter().fold(api::CharFilter(vec![]), |cf, lx| {
          let lxcf = mk_char_filter(lx.char_filter().iter().cloned());
          char_filter_union(&cf, &lxcf)
        });
        let mut lazy_mems = HashMap::new();
        let ctx = SysCtx{
          cted: cted.clone(),
          id: new_sys.id,
          logger: logger.clone(),
          reqnot: req.reqnot()
        };
        let mut tia_ctx = TIACtxImpl{
          lazy: &mut lazy_mems,
          sys: ctx.clone(),
          basepath: &[],
          path: Substack::Bottom,
        };
        let const_root = (cted.inst().dyn_env().into_iter())
          .map(|(k, v)| (k.marker(), v.into_api(&mut tia_ctx)))
          .collect();
        systems.lock().unwrap().insert(new_sys.id, SystemRecord {
          declfs: cted.inst().dyn_vfs().to_api_rec(&mut vfses),
          vfses,
          cted,
          lazy_members: lazy_mems
        });
        req.handle(new_sys, &api::SystemInst {
          lex_filter,
          const_root,
          line_types: vec![]
        })
      }
      api::HostExtReq::GetMember(get_tree@api::GetMember(sys_id, tree_id)) => {
        let mut systems_g = systems.lock().unwrap();
        let sys = systems_g.get_mut(sys_id).expect("System not found");
        let lazy = &mut sys.lazy_members;
        let (path, cb) = match lazy.insert(*tree_id, MemberRecord::Res) {
          None => panic!("Tree for ID not found"),
          Some(MemberRecord::Res) => panic!("This tree has already been transmitted"),
          Some(MemberRecord::Gen(path, cb)) => (path, cb),
        };
        let tree = cb.build(path.clone());
        req.handle(get_tree, &tree.into_api(&mut TIACtxImpl{
            sys: SysCtx::new(*sys_id, &sys.cted, &logger, req.reqnot()),
            path: Substack::Bottom,
            basepath: &path,
            lazy,
        }))
      }
      api::HostExtReq::VfsReq(api::VfsReq::GetVfs(get_vfs@api::GetVfs(sys_id))) => {
        let systems_g = systems.lock().unwrap();
        req.handle(get_vfs, &systems_g[sys_id].declfs)
      }
      api::HostExtReq::VfsReq(api::VfsReq::VfsRead(vfs_read)) => {
        let api::VfsRead(sys_id, vfs_id, path) = vfs_read;
        let systems_g = systems.lock().unwrap();
        let path = path.iter().map(|t| deintern(*t)).collect_vec();
        req.handle(vfs_read, &systems_g[sys_id].vfses[vfs_id].load(PathSlice::new(&path)))
      }
      api::HostExtReq::ParserReq(api::ParserReq::LexExpr(lex)) => {
        let api::LexExpr{ sys, text, pos, id } = *lex;
        let systems_g = systems.lock().unwrap();
        let lexers = systems_g[&sys].cted.inst().dyn_lexers();
        mem::drop(systems_g);
        let text = deintern(text);
        let ctx = LexContext { sys, id, pos, reqnot: req.reqnot(), text: &text };
        let trigger_char = text.chars().nth(pos as usize).unwrap();
        for lx in lexers.iter().filter(|l| char_filter_match(l.char_filter(), trigger_char)) {
          match lx.lex(&text[pos as usize..], &ctx) {
            Err(e) if e.iter().any(|e| *e == err_lexer_na()) => continue,
            Err(e) => {
              let errv = errv_to_apiv(e.iter().filter(|e| **e == err_cascade()));
              return req.handle(lex, &if errv.is_empty() { None } else { Some(Err(errv))})
            },
            Ok((s, expr)) => {
              let ctx = mk_ctx(sys, req.reqnot());
              let expr = expr.to_api(&mut |f, r| do_extra(f, r, ctx.clone()));
              let pos = (text.len() - s.len()) as u32;
              return req.handle(lex, &Some(Ok(api::LexedExpr{ pos, expr })))
            }
          }
        }
        writeln!(logger, "Got notified about n/a character '{trigger_char}'");
        req.handle(lex, &None)
      },
      api::HostExtReq::ParserReq(api::ParserReq::ParseLine(pline@api::ParseLine{ sys, line })) => {
        let mut ctx = mk_ctx(*sys, req.reqnot());
        let parsers = ctx.cted.inst().dyn_parsers();
        let line: Vec<GenTokTree> = ttv_from_api(line, &mut ctx);
        let snip = Snippet::new(line.first().expect("Empty line"), &line);
        let (head, tail) = snip.pop_front().unwrap();
        let name = if let GenTok::Name(n) = &head.tok { n } else { panic!("No line head") };
        let parser = parsers.iter().find(|p| p.line_head() == **name).expect("No parser candidate");
        let o_line = match parser.parse(tail) {
          Err(e) => Err(errv_to_apiv(e.iter())),
          Ok(t) => Ok(ttv_to_api(t, &mut |f, range| {
            api::TokenTree{ range, token: api::Token::Atom(f.clone().build(ctx.clone())) }
          })),
        };
        req.handle(pline, &o_line)
      }
      api::HostExtReq::AtomReq(atom_req) => {
        let atom = atom_req.get_atom();
        with_atom_record(&mk_ctx, req.reqnot(), atom, |nfo, ctx, id, buf| {
          let actx = AtomCtx(buf, atom.drop, ctx.clone());
          match atom_req {
            api::AtomReq::SerializeAtom(ser) => {
              let mut buf = enc_vec(&id);
              let refs = nfo.serialize(actx, &mut buf);
              req.handle(ser, &(buf, refs))
            }
            api::AtomReq::AtomPrint(print@api::AtomPrint(_)) => req.handle(print, &nfo.print(actx)),
            api::AtomReq::AtomSame(same@api::AtomSame(_, r)) => {
              // different systems or different type tags
              if atom.owner != r.owner || buf != &r.data[..8] {
                return req.handle(same, &false)
              }
              req.handle(same, &nfo.same(actx, r))
            },
            api::AtomReq::Fwded(fwded@api::Fwded(_, payload)) => {
              let mut reply = Vec::new();
              nfo.handle_req(actx, &mut &payload[..], &mut reply);
              req.handle(fwded, &reply)
            }
            api::AtomReq::CallRef(call@api::CallRef(_, arg))
              => req.handle(call, &nfo.call_ref(actx, *arg).to_api(ctx.clone())),
            api::AtomReq::FinalCall(call@api::FinalCall(_, arg))
              => req.handle(call, &nfo.call(actx, *arg).to_api(ctx.clone())),
            api::AtomReq::Command(cmd@api::Command(_)) => req.handle(cmd, &match nfo.command(actx) {
              Err(e) => Err(errv_to_apiv(e.iter())),
              Ok(opt) => Ok(match opt {
                Some(cont) => api::NextStep::Continue(cont.into_api(ctx.clone())),
                None => api::NextStep::Halt,
              })
            })
          }
        })
      },
      api::HostExtReq::DeserAtom(deser@DeserAtom(sys, buf, refs)) => {
        let mut read = &mut &buf[..];
        let ctx = mk_ctx(*sys, req.reqnot());
        let id = api::AtomId::decode(&mut read);
        let inst = ctx.cted.inst();
        let nfo = atom_by_idx(inst.card(), id).expect("Deserializing atom with invalid ID");
        req.handle(deser, &nfo.deserialize(ctx.clone(), read, refs))
      }
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    let rcvd = recv_parent_msg().unwrap();
    rn.receive(rcvd)
  }
}
