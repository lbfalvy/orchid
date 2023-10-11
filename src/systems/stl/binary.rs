use std::fmt::Debug;
use std::sync::Arc;

use itertools::Itertools;

use crate::error::RuntimeError;
use crate::foreign::{
  xfn_1ary, xfn_2ary, xfn_3ary, xfn_4ary, Atomic, InertAtomic, XfnResult,
};
use crate::interpreted::Clause;
use crate::systems::codegen::{opt, tuple};
use crate::utils::{iter_find, unwrap_or};
use crate::{ConstTree, Interner};

const INT_BYTES: usize = usize::BITS as usize / 8;

/// A block of binary data
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Binary(pub Arc<Vec<u8>>);
impl InertAtomic for Binary {
  fn type_str() -> &'static str { "a binary blob" }
}

impl Debug for Binary {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut iter = self.0.iter().copied();
    f.write_str("Binary")?;
    for mut chunk in iter.by_ref().take(32).chunks(4).into_iter() {
      let a = chunk.next().expect("Chunks cannot be empty");
      let b = unwrap_or!(chunk.next(); return write!(f, "{a:02x}"));
      let c = unwrap_or!(chunk.next(); return write!(f, "{a:02x}{b:02x}"));
      let d =
        unwrap_or!(chunk.next(); return write!(f, "{a:02x}{b:02x}{c:02x}"));
      write!(f, "{a:02x}{b:02x}{c:02x}{d:02x}")?
    }
    if iter.next().is_some() { write!(f, "...") } else { Ok(()) }
  }
}

/// Append two binary data blocks
pub fn concatenate(a: Binary, b: Binary) -> XfnResult<Binary> {
  let data = a.0.iter().chain(b.0.iter()).copied().collect();
  Ok(Binary(Arc::new(data)))
}

/// Extract a subsection of the binary data
pub fn slice(s: Binary, i: usize, len: usize) -> XfnResult<Binary> {
  if i + len < s.0.len() {
    RuntimeError::fail(
      "Byte index out of bounds".to_string(),
      "indexing binary",
    )?
  }
  Ok(Binary(Arc::new(s.0[i..i + len].to_vec())))
}

/// Return the index where the first argument first contains the second, if any
pub fn find(haystack: Binary, needle: Binary) -> XfnResult<Clause> {
  let found = iter_find(haystack.0.iter(), needle.0.iter());
  Ok(opt(found.map(usize::atom_exi)))
}

/// Split binary data block into two smaller blocks
pub fn split(bin: Binary, i: usize) -> XfnResult<Clause> {
  if bin.0.len() < i {
    RuntimeError::fail(
      "Byte index out of bounds".to_string(),
      "splitting binary",
    )?
  }
  let (asl, bsl) = bin.0.split_at(i);
  Ok(tuple([asl, bsl].map(|s| Binary(Arc::new(s.to_vec())).atom_exi())))
}

/// Read a number from a binary blob
pub fn get_num(
  buf: Binary,
  loc: usize,
  size: usize,
  is_le: bool,
) -> XfnResult<usize> {
  if buf.0.len() < (loc + size) {
    RuntimeError::fail(
      "section out of range".to_string(),
      "reading number from binary data",
    )?
  }
  if INT_BYTES < size {
    RuntimeError::fail(
      "more than std::bin::int_bytes bytes provided".to_string(),
      "reading number from binary data",
    )?
  }
  let mut data = [0u8; INT_BYTES];
  let section = &buf.0[loc..(loc + size)];
  let num = if is_le {
    data[0..size].copy_from_slice(section);
    usize::from_le_bytes(data)
  } else {
    data[INT_BYTES - size..].copy_from_slice(section);
    usize::from_be_bytes(data)
  };
  Ok(num)
}

/// Convert a number into a blob
pub fn from_num(size: usize, is_le: bool, data: usize) -> XfnResult<Binary> {
  if INT_BYTES < size {
    RuntimeError::fail(
      "more than std::bin::int_bytes bytes requested".to_string(),
      "converting number to binary",
    )?
  }
  let bytes = match is_le {
    true => data.to_le_bytes()[0..size].to_vec(),
    false => data.to_be_bytes()[8 - size..].to_vec(),
  };
  Ok(Binary(Arc::new(bytes)))
}

/// Detect the number of bytes in the blob
pub fn size(b: Binary) -> XfnResult<usize> { Ok(b.0.len()) }

pub fn bin(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("binary"),
    ConstTree::tree([
      (i.i("concat"), ConstTree::xfn(xfn_2ary(concatenate))),
      (i.i("slice"), ConstTree::xfn(xfn_3ary(slice))),
      (i.i("find"), ConstTree::xfn(xfn_2ary(find))),
      (i.i("split"), ConstTree::xfn(xfn_2ary(split))),
      (i.i("get_num"), ConstTree::xfn(xfn_4ary(get_num))),
      (i.i("from_num"), ConstTree::xfn(xfn_3ary(from_num))),
      (i.i("size"), ConstTree::xfn(xfn_1ary(size))),
      (i.i("int_bytes"), ConstTree::atom(INT_BYTES)),
    ]),
  )])
}
