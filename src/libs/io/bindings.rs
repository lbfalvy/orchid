use super::flow::IOCmdHandlePack;
use super::instances::{BRead, ReadCmd, SRead, WriteCmd};
use super::service::{Sink, Source};
use crate::foreign::cps_box::CPSBox;
use crate::foreign::error::ExternResult;
use crate::foreign::inert::Inert;
use crate::gen::tree::{xfn_ent, ConstTree};
use crate::libs::scheduler::system::SharedHandle;
use crate::libs::std::binary::Binary;
use crate::libs::std::runtime_error::RuntimeError;
use crate::libs::std::string::OrcString;
use crate::utils::combine::Combine;

pub type WriteHandle = Inert<SharedHandle<Sink>>;
pub type ReadHandle = Inert<SharedHandle<Source>>;

type ReadCmdPack = CPSBox<IOCmdHandlePack<ReadCmd>>;
type WriteCmdPack = CPSBox<IOCmdHandlePack<WriteCmd>>;

pub fn read_string(Inert(handle): ReadHandle) -> ReadCmdPack {
  let cmd = ReadCmd::RStr(SRead::All);
  CPSBox::new(3, IOCmdHandlePack { handle, cmd })
}
pub fn read_line(Inert(handle): ReadHandle) -> ReadCmdPack {
  let cmd = ReadCmd::RStr(SRead::Line);
  CPSBox::new(3, IOCmdHandlePack { handle, cmd })
}
pub fn read_bin(Inert(handle): ReadHandle) -> ReadCmdPack {
  let cmd = ReadCmd::RBytes(BRead::All);
  CPSBox::new(3, IOCmdHandlePack { handle, cmd })
}
pub fn read_bytes(Inert(handle): ReadHandle, n: Inert<usize>) -> ReadCmdPack {
  let cmd = ReadCmd::RBytes(BRead::N(n.0));
  CPSBox::new(3, IOCmdHandlePack { cmd, handle })
}
pub fn read_until(
  Inert(handle): ReadHandle,
  Inert(pattern): Inert<usize>,
) -> ExternResult<ReadCmdPack> {
  let pattern = pattern.try_into().map_err(|_| {
    let msg = format!("{pattern} doesn't fit into a byte");
    RuntimeError::ext(msg, "converting number to byte")
  })?;
  let cmd = ReadCmd::RBytes(BRead::Until(pattern));
  Ok(CPSBox::new(3, IOCmdHandlePack { handle, cmd }))
}
pub fn write_str(Inert(handle): WriteHandle, string: Inert<OrcString>) -> WriteCmdPack {
  let cmd = WriteCmd::WStr(string.0.get_string());
  CPSBox::new(3, IOCmdHandlePack { handle, cmd })
}
pub fn write_bin(Inert(handle): WriteHandle, bytes: Inert<Binary>) -> WriteCmdPack {
  CPSBox::new(3, IOCmdHandlePack { handle, cmd: WriteCmd::WBytes(bytes.0) })
}
pub fn flush(Inert(handle): WriteHandle) -> WriteCmdPack {
  CPSBox::new(3, IOCmdHandlePack { handle, cmd: WriteCmd::Flush })
}

pub fn io_bindings<'a>(std_streams: impl IntoIterator<Item = (&'a str, ConstTree)>) -> ConstTree {
  ConstTree::ns("system::io", [ConstTree::tree([
    xfn_ent("read_string", [read_string]),
    xfn_ent("read_line", [read_line]),
    xfn_ent("read_bin", [read_bin]),
    xfn_ent("read_n_bytes", [read_bytes]),
    xfn_ent("read_until", [read_until]),
    xfn_ent("write_str", [write_str]),
    xfn_ent("write_bin", [write_bin]),
    xfn_ent("flush", [flush]),
  ])
  .combine(ConstTree::tree(std_streams))
  .expect("std_stream name clashing with io functions")])
}
