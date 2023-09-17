use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::facade::{IntoSystem, System};
use crate::foreign::cps_box::{init_cps, CPSBox};
use crate::foreign::{Atomic, InertAtomic};
use crate::interpreted::{Clause, ExprInst};
use crate::interpreter::HandlerTable;
use crate::systems::codegen::{call, list, orchid_opt, tuple};
use crate::systems::io::wrap_io_error;
use crate::systems::scheduler::{SeqScheduler, SharedHandle};
use crate::systems::stl::Boolean;
use crate::systems::RuntimeError;
use crate::utils::unwrap_or;
use crate::{define_fn, ConstTree, OrcString};

#[derive(Debug, Clone)]
pub struct ReadFileCmd(OrcString);
impl InertAtomic for ReadFileCmd {
  fn type_str() -> &'static str { "readfile command" }
}

#[derive(Debug, Clone)]
pub struct ReadDirCmd(OrcString);
impl InertAtomic for ReadDirCmd {
  fn type_str() -> &'static str { "readdir command" }
}

#[derive(Debug, Clone)]
pub struct WriteFile {
  name: OrcString,
  append: bool,
}
impl InertAtomic for WriteFile {
  fn type_str() -> &'static str { "writefile command" }
}

#[must_use]
fn read_file(sched: &SeqScheduler, cmd: CPSBox<ReadFileCmd>) -> ExprInst {
  let (ReadFileCmd(name), succ, fail, cont) = cmd.unpack3();
  let name = name.get_string();
  let cancel = sched.run_orphan(
    move |_| File::open(name),
    |file, _| match file {
      Err(e) => vec![call(fail, [wrap_io_error(e)]).wrap()],
      Ok(f) => {
        let source =
          SharedHandle::wrap(BufReader::new(Box::new(f) as Box<dyn Read>));
        vec![call(succ, [source.atom_exi()]).wrap()]
      },
    },
  );
  call(cont, [init_cps(1, cancel).wrap()]).wrap()
}

#[must_use]
fn read_dir(sched: &SeqScheduler, cmd: CPSBox<ReadDirCmd>) -> ExprInst {
  let (ReadDirCmd(name), succ, fail, cont) = cmd.unpack3();
  let name = name.get_string();
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
          .map(|(n, d)| {
            Ok(tuple([os_str_cls(n)?.wrap(), Boolean(d).atom_exi()]).wrap())
          })
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
  let name = name.get_string();
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

#[derive(Debug, Clone)]
pub struct InvalidString(OsString);
impl InertAtomic for InvalidString {
  fn type_str() -> &'static str { "invalidstring error" }
}

fn os_str_cls(str: OsString) -> Result<Clause, Clause> {
  (str.into_string())
    .map_err(|e| InvalidString(e).atom_cls())
    .map(|s| OrcString::from(s).cls())
}

define_fn! {
  pub IsInvalidString = |x| {
    Ok(Boolean(x.downcast::<InvalidString>().is_ok()).atom_cls())
  };
  pub OpenFileRead = |x| Ok(init_cps(3, ReadFileCmd(x.downcast()?)));
  pub ReadDir = |x| Ok(init_cps(3, ReadDirCmd(x.downcast()?)));
  pub OpenFileWrite = |x| {
    Ok(init_cps(3, WriteFile{ name: x.downcast()?, append: false }))
  };
  pub OpenFileAppend = |x| {
    Ok(init_cps(3, WriteFile{ name: x.downcast()?, append: true }))
  };

  pub JoinPaths { root: OrcString, sub: OrcString } => {
    let res = Path::new(root.as_str())
      .join(sub.as_str())
      .into_os_string();
    os_str_cls(res.clone()).map_err(|_| RuntimeError::ext(
      format!("result {res:?} contains illegal characters"),
      "joining paths"
    ))
  };
  pub PopPath = |x| {
    eprintln!("argument is {x}");
    let arg = x.downcast::<OrcString>()?;
    let full_path = Path::new(arg.as_str());
    let parent = unwrap_or! {full_path.parent(); {
      return Ok(orchid_opt(None))
    }};
    let sub = unwrap_or! {full_path.file_name(); {
      return Ok(orchid_opt(None))
    }};
    Ok(orchid_opt(Some(tuple(
      [parent.as_os_str(), sub]
        .into_iter()
        .map(|s| os_str_cls(s.to_owned()).map_err(|_| RuntimeError::ext(
          format!("Result {s:?} contains illegal characters"),
          "splitting a path"
        )))
        .map_ok(Clause::wrap)
        .collect::<Result<Vec<_>, _>>()?
    ).wrap())))
  }
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
          (i.i("is_invalid_string"), ConstTree::xfn(IsInvalidString)),
          (i.i("readfile"), ConstTree::xfn(OpenFileRead)),
          (i.i("readdir"), ConstTree::xfn(ReadDir)),
          (i.i("writefile"), ConstTree::xfn(OpenFileWrite)),
          (i.i("appendfile"), ConstTree::xfn(OpenFileAppend)),
          (i.i("join_paths"), ConstTree::xfn(JoinPaths)),
          (i.i("pop_path"), ConstTree::xfn(PopPath)),
        ]),
      )
      .unwrap_tree(),
      handlers,
    }
  }
}
