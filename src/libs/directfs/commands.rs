use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};

use super::osstring::os_string_lib;
use crate::facade::system::{IntoSystem, System};
use crate::foreign::atom::Atomic;
use crate::foreign::cps_box::CPSBox;
use crate::foreign::error::ExternResult;
use crate::foreign::fn_bridge::constructors::{xfn_1ary, xfn_2ary};
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::process::Unstable;
use crate::foreign::to_clause::ToClause;
use crate::gen::tpl;
use crate::gen::traits::Gen;
use crate::gen::tree::{atom_ent, atom_leaf, ConstTree};
use crate::interpreter::gen_nort::nort_gen;
use crate::interpreter::handler::HandlerTable;
use crate::interpreter::nort::{Clause, Expr};
use crate::libs::io::instances::io_error_handler;
use crate::libs::io::{Sink, Source};
use crate::libs::scheduler::system::{SeqScheduler, SharedHandle};
use crate::libs::std::runtime_error::RuntimeError;
use crate::utils::combine::Combine;
use crate::virt_fs::DeclTree;

#[derive(Debug, Clone)]
struct ReadFileCmd(OsString);
impl InertPayload for ReadFileCmd {
  const TYPE_STR: &'static str = "readfile command";
}

#[derive(Debug, Clone)]
struct ReadDirCmd(OsString);
impl InertPayload for ReadDirCmd {
  const TYPE_STR: &'static str = "readdir command";
}

#[derive(Debug, Clone)]
struct WriteFile {
  name: OsString,
  append: bool,
}
impl InertPayload for WriteFile {
  const TYPE_STR: &'static str = "writefile command";
}

#[must_use]
fn read_file(sched: &SeqScheduler, cmd: &CPSBox<ReadFileCmd>) -> Expr {
  let (ReadFileCmd(name), succ, fail, cont) = cmd.unpack3();
  let name = name.clone();
  let cancel = sched.run_orphan(
    move |_| File::open(name),
    |file, _| match file {
      Err(e) => vec![io_error_handler(e, fail)],
      Ok(f) => {
        let source_handle = SharedHandle::wrap(Source::new(Box::new(f)));
        let tpl = tpl::A(tpl::Slot, tpl::V(Inert(source_handle)));
        vec![tpl.template(nort_gen(succ.location()), [succ])]
      },
    },
  );
  let tpl = tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel)));
  tpl.template(nort_gen(cont.location()), [cont])
}

#[must_use]
fn read_dir(sched: &SeqScheduler, cmd: &CPSBox<ReadDirCmd>) -> Expr {
  let (ReadDirCmd(name), succ, fail, cont) = cmd.unpack3();
  let name = name.clone();
  let cancel = sched.run_orphan(
    move |_| {
      Path::new(&name)
        .read_dir()?
        .map(|r| r.and_then(|e| Ok((e.file_name(), e.file_type()?.is_dir()))))
        .collect()
    },
    |items: std::io::Result<Vec<(OsString, bool)>>, _| match items {
      Err(e) => vec![io_error_handler(e, fail)],
      Ok(os_namev) => {
        let converted = (os_namev.into_iter())
          .map(|(n, d)| {
            Ok((
              Inert(n).atom_expr(succ.location()),
              Inert(d).atom_expr(succ.location()),
            ))
          })
          .collect::<Result<Vec<_>, Clause>>();
        match converted {
          Err(e) => {
            let e = e.to_expr(fail.location());
            let tpl = tpl::A(tpl::Slot, tpl::Slot);
            vec![tpl.template(nort_gen(fail.location()), [fail, e])]
          },
          Ok(names) => {
            let names = names.to_expr(succ.location());
            let tpl = tpl::A(tpl::Slot, tpl::Slot);
            vec![tpl.template(nort_gen(succ.location()), [succ, names])]
          },
        }
      },
    },
  );
  let tpl = tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel)));
  tpl.template(nort_gen(cont.location()), [cont])
}

#[must_use]
fn write_file(sched: &SeqScheduler, cmd: &CPSBox<WriteFile>) -> Expr {
  let (cmd, succ, fail, cont) = cmd.unpack3();
  let cmd = cmd.clone();
  let cancel = sched.run_orphan(
    move |_| File::options().write(true).append(cmd.append).open(&cmd.name),
    |file, _| match file {
      Err(e) => vec![io_error_handler(e, fail)],
      Ok(f) => {
        let sink_handle = SharedHandle::wrap(Box::new(f) as Sink);
        let tpl = tpl::A(tpl::Slot, tpl::V(Inert(sink_handle)));
        vec![tpl.template(nort_gen(succ.location()), [succ])]
      },
    },
  );
  let tpl = tpl::A(tpl::Slot, tpl::V(CPSBox::new(1, cancel)));
  tpl.template(nort_gen(cont.location()), [cont])
}

fn open_file_read_cmd(name: OsString) -> CPSBox<ReadFileCmd> {
  CPSBox::new(3, ReadFileCmd(name))
}

fn read_dir_cmd(name: OsString) -> CPSBox<ReadDirCmd> {
  CPSBox::new(3, ReadDirCmd(name))
}

fn open_file_write_cmd(name: OsString) -> CPSBox<WriteFile> {
  CPSBox::new(3, WriteFile { name, append: false })
}

fn open_file_append_cmd(name: OsString) -> CPSBox<WriteFile> {
  CPSBox::new(3, WriteFile { name, append: true })
}

fn join_paths(root: OsString, sub: OsString) -> OsString {
  let mut path = PathBuf::from(root);
  path.push(sub);
  path.into_os_string()
}

fn pop_path(
  path: Inert<OsString>,
) -> Option<(Inert<OsString>, Inert<OsString>)> {
  let mut path = PathBuf::from(path.0);
  let sub = path.file_name()?.to_owned();
  debug_assert!(path.pop(), "file_name above returned Some");
  Some((Inert(path.into_os_string()), Inert(sub)))
}

/// A rudimentary system to read and write files.
#[derive(Clone)]
pub struct DirectFS {
  scheduler: SeqScheduler,
}
impl DirectFS {
  /// Create a new instance of the system.
  pub fn new(scheduler: SeqScheduler) -> Self { Self { scheduler } }
}

impl IntoSystem<'static> for DirectFS {
  fn into_system(self) -> System<'static> {
    let mut handlers = HandlerTable::new();
    let sched = self.scheduler.clone();
    handlers.register(move |cmd| read_file(&sched, cmd));
    let sched = self.scheduler.clone();
    handlers.register(move |cmd| read_dir(&sched, cmd));
    let sched = self.scheduler;
    handlers.register(move |cmd| write_file(&sched, cmd));
    System {
      name: "system::directfs",
      code: DeclTree::empty(),
      prelude: Vec::new(),
      lexer_plugins: vec![],
      line_parsers: vec![],
      constants: ConstTree::ns("system::fs", [ConstTree::tree([
        ("read_file", atom_leaf(xfn_1ary(open_file_read_cmd))),
        ("read_dir", atom_leaf(xfn_1ary(read_dir_cmd))),
        ("write_file", atom_leaf(xfn_1ary(open_file_write_cmd))),
        ("append_file", atom_leaf(xfn_1ary(open_file_append_cmd))),
        ("join_paths", atom_leaf(xfn_2ary(join_paths))),
        ("pop_path", atom_leaf(xfn_1ary(pop_path))),
        atom_ent("cwd", [Unstable::new(|_| -> ExternResult<_> {
          let path = std::env::current_dir()
            .map_err(|e| RuntimeError::ext(e.to_string(), "reading CWD"))?;
          Ok(Inert(path.into_os_string()))
        })]),
      ])])
      .combine(os_string_lib())
      .expect("os_string library and directfs conflict"),
      handlers,
    }
  }
}
