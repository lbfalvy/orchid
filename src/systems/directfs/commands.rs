use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use hashbrown::HashMap;
use itertools::Itertools;

use super::osstring::os_string_lib;
use crate::ddispatch::Responder;
use crate::facade::{IntoSystem, System};
use crate::foreign::cps_box::{init_cps, CPSBox};
use crate::foreign::{
  xfn_1ary, xfn_2ary, Atomic, AtomicReturn, InertAtomic, XfnResult,
};
use crate::interpreted::{Clause, ExprInst};
use crate::interpreter::HandlerTable;
use crate::systems::codegen::{call, list, opt, tuple};
use crate::systems::io::{wrap_io_error, Source};
use crate::systems::scheduler::{SeqScheduler, SharedHandle};
use crate::systems::stl::Boolean;
use crate::systems::RuntimeError;
use crate::utils::unwrap_or;
use crate::ConstTree;

#[derive(Debug, Clone)]
pub struct CurrentDir;
impl Responder for CurrentDir {}
impl Atomic for CurrentDir {
  fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
  fn as_any_ref(&self) -> &dyn std::any::Any { self }
  fn run(
    self: Box<Self>,
    ctx: crate::interpreter::Context,
  ) -> crate::foreign::AtomicResult {
    let cwd = std::env::current_dir()
      .map_err(|e| RuntimeError::ext(e.to_string(), "reading CWD"))?;
    Ok(AtomicReturn {
      clause: cwd.into_os_string().atom_cls(),
      gas: ctx.gas.map(|g| g - 1),
      inert: false,
    })
  }
}

#[derive(Debug, Clone)]
pub struct ReadFileCmd(OsString);
impl InertAtomic for ReadFileCmd {
  fn type_str() -> &'static str { "readfile command" }
}

#[derive(Debug, Clone)]
pub struct ReadDirCmd(OsString);
impl InertAtomic for ReadDirCmd {
  fn type_str() -> &'static str { "readdir command" }
}

#[derive(Debug, Clone)]
pub struct WriteFile {
  name: OsString,
  append: bool,
}
impl InertAtomic for WriteFile {
  fn type_str() -> &'static str { "writefile command" }
}

#[must_use]
fn read_file(sched: &SeqScheduler, cmd: CPSBox<ReadFileCmd>) -> ExprInst {
  let (ReadFileCmd(name), succ, fail, cont) = cmd.unpack3();
  let cancel = sched.run_orphan(
    move |_| File::open(name),
    |file, _| match file {
      Err(e) => vec![call(fail, [wrap_io_error(e)]).wrap()],
      Ok(f) => {
        let source: Source =
          BufReader::new(Box::new(f) as Box<dyn Read + Send>);
        vec![call(succ, [SharedHandle::wrap(source).atom_exi()]).wrap()]
      },
    },
  );
  call(cont, [init_cps(1, cancel).wrap()]).wrap()
}

#[must_use]
fn read_dir(sched: &SeqScheduler, cmd: CPSBox<ReadDirCmd>) -> ExprInst {
  let (ReadDirCmd(name), succ, fail, cont) = cmd.unpack3();
  let cancel = sched.run_orphan(
    move |_| {
      Path::new(&name)
        .read_dir()?
        .map(|r| r.and_then(|e| Ok((e.file_name(), e.file_type()?.is_dir()))))
        .collect()
    },
    |items: std::io::Result<Vec<(OsString, bool)>>, _| match items {
      Err(e) => vec![call(fail, [wrap_io_error(e)]).wrap()],
      Ok(os_namev) => {
        let converted = (os_namev.into_iter())
          .map(|(n, d)| Ok(tuple([n.atom_exi(), Boolean(d).atom_exi()]).wrap()))
          .collect::<Result<Vec<_>, Clause>>();
        match converted {
          Err(e) => vec![call(fail, [e.wrap()]).wrap()],
          Ok(names) => vec![call(succ, [list(names).wrap()]).wrap()],
        }
      },
    },
  );
  call(cont, [init_cps(1, cancel).wrap()]).wrap()
}

#[must_use]
pub fn write_file(sched: &SeqScheduler, cmd: CPSBox<WriteFile>) -> ExprInst {
  let (WriteFile { name, append }, succ, fail, cont) = cmd.unpack3();
  let cancel = sched.run_orphan(
    move |_| File::options().write(true).append(append).open(name),
    |file, _| match file {
      Err(e) => vec![call(fail, [wrap_io_error(e)]).wrap()],
      Ok(f) => {
        let handle = SharedHandle::wrap(Box::new(f) as Box<dyn Write>);
        vec![call(succ, [handle.atom_exi()]).wrap()]
      },
    },
  );
  call(cont, [init_cps(1, cancel).wrap()]).wrap()
}

pub fn open_file_read_cmd(name: OsString) -> XfnResult<Clause> {
  Ok(init_cps(3, ReadFileCmd(name)))
}

pub fn read_dir_cmd(name: OsString) -> XfnResult<Clause> {
  Ok(init_cps(3, ReadDirCmd(name)))
}

pub fn open_file_write_cmd(name: OsString) -> XfnResult<Clause> {
  Ok(init_cps(3, WriteFile { name, append: false }))
}

pub fn open_file_append_cmd(name: OsString) -> XfnResult<Clause> {
  Ok(init_cps(3, WriteFile { name, append: true }))
}

pub fn join_paths(root: OsString, sub: OsString) -> XfnResult<OsString> {
  let mut path = PathBuf::from(root);
  path.push(sub);
  Ok(path.into_os_string())
}

pub fn pop_path(path: OsString) -> XfnResult<Clause> {
  let mut path = PathBuf::from(path);
  let sub = unwrap_or! {path.file_name(); {
    return Ok(opt(None))
  }}
  .to_owned();
  debug_assert!(path.pop(), "file_name above returned Some");
  Ok(opt(Some(
    tuple([path.into_os_string().atom_exi(), sub.atom_exi()]).wrap(),
  )))
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
  fn into_system(self, i: &crate::Interner) -> System<'static> {
    let mut handlers = HandlerTable::new();
    let sched = self.scheduler.clone();
    handlers.register(move |cmd| Ok(read_file(&sched, *cmd)));
    let sched = self.scheduler.clone();
    handlers.register(move |cmd| Ok(read_dir(&sched, *cmd)));
    let sched = self.scheduler;
    handlers.register(move |cmd| Ok(write_file(&sched, *cmd)));
    System {
      name: ["system", "directfs"].into_iter().map_into().collect(),
      code: HashMap::new(),
      prelude: Vec::new(),
      constants: ConstTree::namespace(
        [i.i("system"), i.i("directfs")],
        ConstTree::tree([
          (i.i("read_file"), ConstTree::xfn(xfn_1ary(open_file_read_cmd))),
          (i.i("read_dir"), ConstTree::xfn(xfn_1ary(read_dir_cmd))),
          (i.i("write_file"), ConstTree::xfn(xfn_1ary(open_file_write_cmd))),
          (i.i("append_file"), ConstTree::xfn(xfn_1ary(open_file_append_cmd))),
          (i.i("join_paths"), ConstTree::xfn(xfn_2ary(join_paths))),
          (i.i("pop_path"), ConstTree::xfn(xfn_1ary(pop_path))),
          (i.i("cwd"), ConstTree::atom(CurrentDir)),
        ]) + os_string_lib(i),
      )
      .unwrap_tree(),
      handlers,
    }
  }
}
