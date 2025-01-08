use std::io::Write;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{mem, process, thread};

use hashbrown::HashMap;
use itertools::Itertools;
use orchid_api_traits::{enc_vec, Decode, Encode};
use orchid_base::char_filter::{char_filter_match, char_filter_union, mk_char_filter};
use orchid_base::clone;
use orchid_base::interner::{init_replica, sweep_replica, Tok};
use orchid_base::logging::Logger;
use orchid_base::macros::{mtreev_from_api, mtreev_to_api};
use orchid_base::name::{PathSlice, Sym};
use orchid_base::parse::{Comment, Snippet};
use orchid_base::reqnot::{ReqHandlish, ReqNot, RequestHandle, Requester};
use orchid_base::tree::{ttv_from_api, ttv_to_api};
use substack::Substack;

use crate::api;
use crate::atom::{AtomCtx, AtomDynfo};
use crate::atom_owned::OBJ_STORE;
use crate::fs::VirtFS;
use crate::lexer::{err_cascade, err_not_applicable, LexContext};
use crate::macros::{apply_rule, RuleCtx};
use crate::msg::{recv_parent_msg, send_parent_msg};
use crate::system::{atom_by_idx, SysCtx};
use crate::system_ctor::{CtedObj, DynSystemCtor};
use crate::tree::{do_extra, GenTok, GenTokTree, LazyMemberFactory, TIACtxImpl};

pub type ExtReq = RequestHandle<api::ExtMsgSet>;
pub type ExtReqNot = ReqNot<api::ExtMsgSet>;

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
  cb: impl FnOnce(Box<dyn AtomDynfo>, SysCtx, api::AtomId, &[u8]) -> T,
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
    clone!(systems, logger; move |hand, req| match req {
      api::HostExtReq::Ping(ping@api::Ping) => hand.handle(&ping, &()),
      api::HostExtReq::Sweep(sweep@api::Sweep) => hand.handle(&sweep, &sweep_replica()),
      api::HostExtReq::SysReq(api::SysReq::NewSystem(new_sys)) => {
        let i = decls.iter().enumerate().find(|(_, s)| s.id == new_sys.system).unwrap().0;
        let cted = data.systems[i].new_system(&new_sys);
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
          reqnot: hand.reqnot()
        };
        let mut tia_ctx = TIACtxImpl{
          lazy: &mut lazy_mems,
          sys: ctx.clone(),
          basepath: &[],
          path: Substack::Bottom,
        };
        let const_root = (cted.inst().dyn_env().into_iter())
          .map(|(k, v)| (k.to_api(), v.into_api(&mut tia_ctx)))
          .collect();
        systems.lock().unwrap().insert(new_sys.id, SystemRecord {
          declfs: cted.inst().dyn_vfs().to_api_rec(&mut vfses),
          vfses,
          cted,
          lazy_members: lazy_mems
        });
        hand.handle(&new_sys, &api::SystemInst {
          lex_filter,
          const_root,
          line_types: vec![]
        })
      }
      api::HostExtReq::GetMember(get_tree@api::GetMember(sys_id, tree_id)) => {
        let mut systems_g = systems.lock().unwrap();
        let sys = systems_g.get_mut(&sys_id).expect("System not found");
        let lazy = &mut sys.lazy_members;
        let (path, cb) = match lazy.insert(tree_id, MemberRecord::Res) {
          None => panic!("Tree for ID not found"),
          Some(MemberRecord::Res) => panic!("This tree has already been transmitted"),
          Some(MemberRecord::Gen(path, cb)) => (path, cb),
        };
        let tree = cb.build(path.clone());
        hand.handle(&get_tree, &tree.into_api(&mut TIACtxImpl{
            sys: SysCtx::new(sys_id, &sys.cted, &logger, hand.reqnot()),
            path: Substack::Bottom,
            basepath: &path,
            lazy,
        }))
      }
      api::HostExtReq::VfsReq(api::VfsReq::GetVfs(get_vfs@api::GetVfs(sys_id))) => {
        let systems_g = systems.lock().unwrap();
        hand.handle(&get_vfs, &systems_g[&sys_id].declfs)
      }
      api::HostExtReq::SysReq(api::SysReq::SysFwded(fwd)) => {
        let api::SysFwded(sys_id, payload) = fwd;
        let ctx = mk_ctx(sys_id, hand.reqnot());
        let sys = ctx.cted.inst();
        sys.dyn_request(hand, payload)
      }
      api::HostExtReq::VfsReq(api::VfsReq::VfsRead(vfs_read)) => {
        let api::VfsRead(sys_id, vfs_id, path) = &vfs_read;
        let systems_g = systems.lock().unwrap();
        let path = path.iter().map(|t| Tok::from_api(*t)).collect_vec();
        hand.handle(&vfs_read, &systems_g[sys_id].vfses[vfs_id].load(PathSlice::new(&path)))
      }
      api::HostExtReq::LexExpr(lex @ api::LexExpr{ sys, text, pos, id }) => {
        let systems_g = systems.lock().unwrap();
        let lexers = systems_g[&sys].cted.inst().dyn_lexers();
        mem::drop(systems_g);
        let text = Tok::from_api(text);
        let ctx = LexContext { sys, id, pos, reqnot: hand.reqnot(), text: &text };
        let trigger_char = text.chars().nth(pos as usize).unwrap();
        for lx in lexers.iter().filter(|l| char_filter_match(l.char_filter(), trigger_char)) {
          match lx.lex(&text[pos as usize..], &ctx) {
            Err(e) if e.any(|e| *e == err_not_applicable()) => continue,
            Err(e) => {
              let eopt = e.keep_only(|e| *e != err_cascade()).map(|e| Err(e.to_api()));
              return hand.handle(&lex, &eopt)
            },
            Ok((s, expr)) => {
              let ctx = mk_ctx(sys, hand.reqnot());
              let expr = expr.to_api(&mut |f, r| do_extra(f, r, ctx.clone()));
              let pos = (text.len() - s.len()) as u32;
              return hand.handle(&lex, &Some(Ok(api::LexedExpr{ pos, expr })))
            }
          }
        }
        writeln!(logger, "Got notified about n/a character '{trigger_char}'");
        hand.handle(&lex, &None)
      },
      api::HostExtReq::ParseLine(pline) => {
        let api::ParseLine{ exported, comments, sys, line } = &pline;
        let mut ctx = mk_ctx(*sys, hand.reqnot());
        let parsers = ctx.cted.inst().dyn_parsers();
        let comments = comments.iter().map(Comment::from_api).collect();
        let line: Vec<GenTokTree> = ttv_from_api(line, &mut ctx);
        let snip = Snippet::new(line.first().expect("Empty line"), &line);
        let (head, tail) = snip.pop_front().unwrap();
        let name = if let GenTok::Name(n) = &head.tok { n } else { panic!("No line head") };
        let parser = parsers.iter().find(|p| p.line_head() == **name).expect("No parser candidate");
        let o_line = match parser.parse(*exported, comments, tail) {
          Err(e) => Err(e.to_api()),
          Ok(t) => Ok(ttv_to_api(t, &mut |f, range| {
            api::TokenTree{ range, token: api::Token::Atom(f.clone().build(ctx.clone())) }
          })),
        };
        hand.handle(&pline, &o_line)
      }
      api::HostExtReq::AtomReq(atom_req) => {
        let atom = atom_req.get_atom();
        with_atom_record(&mk_ctx, hand.reqnot(), atom, |nfo, ctx, id, buf| {
          let actx = AtomCtx(buf, atom.drop, ctx.clone());
          match &atom_req {
            api::AtomReq::SerializeAtom(ser) => {
              let mut buf = enc_vec(&id);
              let refs_opt = nfo.serialize(actx, &mut buf);
              hand.handle(ser, &refs_opt.map(|refs| (buf, refs)))
            }
            api::AtomReq::AtomPrint(print@api::AtomPrint(_)) =>
              hand.handle(print, &nfo.print(actx)),
            api::AtomReq::Fwded(fwded) => {
              let api::Fwded(_, key, payload) = &fwded;
              let mut reply = Vec::new();
              let some = nfo.handle_req(actx, Sym::from_api(*key), &mut &payload[..], &mut reply);
              hand.handle(fwded, &some.then_some(reply))
            }
            api::AtomReq::CallRef(call@api::CallRef(_, arg)) => {
              let ret = nfo.call_ref(actx, *arg);
              hand.handle(call, &ret.api_return(ctx.clone(), &mut |h| hand.defer_drop(h)))
            },
            api::AtomReq::FinalCall(call@api::FinalCall(_, arg)) => {
              let ret = nfo.call(actx, *arg);
              hand.handle(call, &ret.api_return(ctx.clone(), &mut |h| hand.defer_drop(h)))
            }
            api::AtomReq::Command(cmd@api::Command(_)) => {
              hand.handle(cmd, &match nfo.command(actx) {
                Err(e) => Err(e.to_api()),
                Ok(opt) => Ok(match opt {
                  None => api::NextStep::Halt,
                  Some(cont) => api::NextStep::Continue(
                    cont.api_return(ctx.clone(), &mut |h| hand.defer_drop(h))
                  ),
                })
              })
            }
          }
        })
      },
      api::HostExtReq::DeserAtom(deser) => {
        let api::DeserAtom(sys, buf, refs) = &deser;
        let mut read = &mut &buf[..];
        let ctx = mk_ctx(*sys, hand.reqnot());
        let id = api::AtomId::decode(&mut read);
        let inst = ctx.cted.inst();
        let nfo = atom_by_idx(inst.card(), id).expect("Deserializing atom with invalid ID");
        hand.handle(&deser, &nfo.deserialize(ctx.clone(), read, refs))
      },
      orchid_api::HostExtReq::ApplyMacro(am) => {
        let tok = hand.will_handle_as(&am);
        let sys_ctx = mk_ctx(am.sys, hand.reqnot());
        let ctx = RuleCtx {
          args: (am.params.into_iter())
            .map(|(k, v)| (
              Tok::from_api(k),
              mtreev_from_api(&v, &mut |_| panic!("No atom in macro prompt!"))
            ))
            .collect(),
          run_id: am.run_id,
          sys: sys_ctx.clone(),
        };
        hand.handle_as(tok, &match apply_rule(am.id, ctx) {
          Err(e) => e.keep_only(|e| *e != err_cascade()).map(|e| Err(e.to_api())),
          Ok(t) => Some(Ok(mtreev_to_api(&t, &mut |a| {
            api::MacroToken::Atom(a.clone().build(sys_ctx.clone()))
          }))),
        })
      }
    }),
  );
  init_replica(rn.clone().map());
  while !exiting.load(Ordering::Relaxed) {
    let rcvd = recv_parent_msg().unwrap();
    rn.receive(&rcvd)
  }
}
