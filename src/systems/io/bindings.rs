use super::flow::IOCmdHandlePack;
use super::instances::{
  BRead, ReadCmd, SRead, WriteCmd, Sink, Source,
};
use crate::foreign::cps_box::init_cps;
use crate::foreign::{Atom, Atomic};
use crate::representations::OrcString;
use crate::systems::scheduler::SharedHandle;
use crate::systems::stl::Binary;
use crate::systems::RuntimeError;
use crate::{ast, define_fn, ConstTree, Interner, Primitive};

define_fn! {
  ReadString = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RStr(SRead::All),
    handle: x.downcast()?
  }));
  ReadLine = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RStr(SRead::Line),
    handle: x.downcast()?
  }));
  ReadBin = |x| Ok(init_cps(3, IOCmdHandlePack{
    cmd: ReadCmd::RBytes(BRead::All),
    handle: x.downcast()?
  }));
  ReadBytes { stream: SharedHandle<Source>, n: u64 } => {
    Ok(init_cps(3, IOCmdHandlePack{
      cmd: ReadCmd::RBytes(BRead::N(n.try_into().unwrap())),
      handle: stream.clone()
    }))
  };
  ReadUntil { stream: SharedHandle<Source>, pattern: u64 } => {
    let delim = pattern.try_into().map_err(|_| RuntimeError::ext(
      "greater than 255".to_string(),
      "converting number to byte"
    ))?;
    Ok(init_cps(3, IOCmdHandlePack{
      cmd: ReadCmd::RBytes(BRead::Until(delim)),
      handle: stream
    }))
  };
  WriteStr { stream: SharedHandle<Sink>, string: OrcString } => {
    Ok(init_cps(3, IOCmdHandlePack {
      cmd: WriteCmd::WStr(string.get_string()),
      handle: stream.clone(),
    }))
  };
  WriteBin { stream: SharedHandle<Sink>, bytes: Binary } => {
    Ok(init_cps(3, IOCmdHandlePack {
      cmd: WriteCmd::WBytes(bytes),
      handle: stream.clone(),
    }))
  };
  Flush = |x| Ok(init_cps(3, IOCmdHandlePack {
    cmd: WriteCmd::Flush,
    handle: x.downcast()?
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
