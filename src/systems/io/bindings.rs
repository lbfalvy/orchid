use super::flow::IOCmdHandlePack;
use super::instances::{
  BRead, ReadCmd, SRead, SinkHandle, SourceHandle, WriteCmd,
};
use crate::foreign::cps_box::init_cps;
use crate::foreign::{Atom, Atomic};
use crate::representations::OrcString;
use crate::systems::stl::Binary;
use crate::systems::RuntimeError;
use crate::{ast, define_fn, ConstTree, Interner, Primitive};

define_fn! {
  ReadString = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RStr(SRead::All),
    handle: x.try_into()?
  }))
}
define_fn! {
  ReadLine = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RStr(SRead::Line),
    handle: x.try_into()?
  }))
}
define_fn! {
  ReadBin = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RBytes(BRead::All),
    handle: x.try_into()?
  }))
}
define_fn! {
  ReadBytes {
    stream: SourceHandle,
    n: u64
  } => Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RBytes(BRead::N((*n).try_into().unwrap())),
    handle: stream.clone()
  }))
}
define_fn! {
  ReadUntil {
    stream: SourceHandle,
    pattern: u64
  } => {
    let delim = (*pattern).try_into().map_err(|_| RuntimeError::ext(
      "greater than 255".to_string(),
      "converting number to byte"
    ))?;
    Ok(init_cps(3, IOCmdHandlePack{
      cmd: ReadCmd::RBytes(BRead::Until(delim)),
      handle: stream.clone()
    }))
  }
}
define_fn! {
  WriteStr {
    stream: SinkHandle,
    string: OrcString
  } => Ok(init_cps(3, IOCmdHandlePack {
    cmd: WriteCmd::WStr(string.get_string()),
    handle: stream.clone(),
  }))
}
define_fn! {
  WriteBin {
    stream: SinkHandle,
    bytes: Binary
  } => Ok(init_cps(3, IOCmdHandlePack {
    cmd: WriteCmd::WBytes(bytes.clone()),
    handle: stream.clone(),
  }))
}
define_fn! {
  Flush = |x| Ok(init_cps(3, IOCmdHandlePack {
    cmd: WriteCmd::Flush,
    handle: x.try_into()?
  }))
}

pub fn io_bindings<'a>(
  i: &Interner,
  std_streams: impl IntoIterator<Item = (&'a str, Box<dyn Atomic>)>,
) -> ConstTree {
  ConstTree::namespace(
    [i.i("system"), i.i("io")],
    ConstTree::tree([
      (i.i("read_string"), ConstTree::xfn(ReadString)),
      (i.i("read_line"), ConstTree::xfn(ReadLine)),
      (i.i("read_bin"), ConstTree::xfn(ReadBin)),
      (i.i("read_n_bytes"), ConstTree::xfn(ReadBytes)),
      (i.i("read_until"), ConstTree::xfn(ReadUntil)),
      (i.i("write_str"), ConstTree::xfn(WriteStr)),
      (i.i("write_bin"), ConstTree::xfn(WriteBin)),
      (i.i("flush"), ConstTree::xfn(Flush)),
    ]) + ConstTree::Tree(
      std_streams
        .into_iter()
        .map(|(n, at)| {
          let expr = ast::Clause::P(Primitive::Atom(Atom(at))).into_expr();
          (i.i(n), ConstTree::Const(expr))
        })
        .collect(),
    ),
  )
}
