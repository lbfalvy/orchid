use std::ffi::OsString;

use crate::foreign::{xfn_1ary, InertAtomic, XfnResult};
use crate::{ConstTree, Interner, OrcString};

impl InertAtomic for OsString {
  fn type_str() -> &'static str { "OsString" }
}

pub fn os_to_string(os: OsString) -> XfnResult<Result<String, OsString>> {
  Ok(os.into_string())
}

pub fn string_to_os(str: OrcString) -> XfnResult<OsString> {
  Ok(str.get_string().into())
}

pub fn os_print(os: OsString) -> XfnResult<String> {
  Ok(os.into_string().unwrap_or_else(|e| e.to_string_lossy().to_string()))
}

pub fn os_string_lib(i: &Interner) -> ConstTree {
  ConstTree::tree([
    (i.i("os_to_string"), ConstTree::xfn(xfn_1ary(os_to_string))),
    (i.i("string_to_os"), ConstTree::xfn(xfn_1ary(string_to_os))),
    (i.i("os_print"), ConstTree::xfn(xfn_1ary(os_print))),
  ])
}
