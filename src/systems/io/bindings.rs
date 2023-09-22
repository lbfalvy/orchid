use super::flow::IOCmdHandlePack;
use super::instances::{BRead, ReadCmd, SRead, Sink, Source, WriteCmd};
use crate::foreign::cps_box::init_cps;
use crate::foreign::{xfn_1ary, xfn_2ary, Atom, Atomic, XfnResult};
use crate::interpreted::Clause;
use crate::representations::OrcString;
use crate::systems::scheduler::SharedHandle;
use crate::systems::stl::Binary;
use crate::systems::RuntimeError;
use crate::{ast, ConstTree, Interner, Primitive};

type WriteHandle = SharedHandle<Sink>;
type ReadHandle = SharedHandle<Source>;

pub fn read_string(handle: ReadHandle) -> XfnResult<Clause> {
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd: ReadCmd::RStr(SRead::All) }))
}
pub fn read_line(handle: ReadHandle) -> XfnResult<Clause> {
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd: ReadCmd::RStr(SRead::Line) }))
}
pub fn read_bin(handle: ReadHandle) -> XfnResult<Clause> {
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd: ReadCmd::RBytes(BRead::All) }))
}
pub fn read_bytes(handle: ReadHandle, n: u64) -> XfnResult<Clause> {
  let cmd = ReadCmd::RBytes(BRead::N(n.try_into().unwrap()));
  Ok(init_cps(3, IOCmdHandlePack { cmd, handle }))
}
pub fn read_until(handle: ReadHandle, pattern: u64) -> XfnResult<Clause> {
  let delim = pattern.try_into().map_err(|_| {
    let msg = "greater than 255".to_string();
    RuntimeError::ext(msg, "converting number to byte")
  })?;
  let cmd = ReadCmd::RBytes(BRead::Until(delim));
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd }))
}
pub fn write_str(handle: WriteHandle, string: OrcString) -> XfnResult<Clause> {
  let cmd = WriteCmd::WStr(string.get_string());
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd }))
}
pub fn write_bin(handle: WriteHandle, bytes: Binary) -> XfnResult<Clause> {
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd: WriteCmd::WBytes(bytes) }))
}
pub fn flush(handle: WriteHandle) -> XfnResult<Clause> {
  Ok(init_cps(3, IOCmdHandlePack { handle, cmd: WriteCmd::Flush }))
}

pub fn io_bindings<'a>(
  i: &Interner,
  std_streams: impl IntoIterator<Item = (&'a str, Box<dyn Atomic>)>,
) -> ConstTree {
  ConstTree::namespace(
    [i.i("system"), i.i("io")],
    ConstTree::tree([
      (i.i("read_string"), ConstTree::xfn(xfn_1ary(read_string))),
      (i.i("read_line"), ConstTree::xfn(xfn_1ary(read_line))),
      (i.i("read_bin"), ConstTree::xfn(xfn_1ary(read_bin))),
      (i.i("read_n_bytes"), ConstTree::xfn(xfn_2ary(read_bytes))),
      (i.i("read_until"), ConstTree::xfn(xfn_2ary(read_until))),
      (i.i("write_str"), ConstTree::xfn(xfn_2ary(write_str))),
      (i.i("write_bin"), ConstTree::xfn(xfn_2ary(write_bin))),
      (i.i("flush"), ConstTree::xfn(xfn_1ary(flush))),
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
