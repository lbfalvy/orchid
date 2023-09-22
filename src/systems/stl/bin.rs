use std::fmt::Debug;
use std::sync::Arc;

use itertools::Itertools;

use super::Boolean;
use crate::foreign::{
  xfn_1ary, xfn_2ary, xfn_3ary, xfn_4ary, Atomic, InertAtomic, XfnResult,
};
use crate::interpreted::Clause;
use crate::systems::codegen::{opt, tuple};
use crate::systems::RuntimeError;
use crate::utils::{iter_find, unwrap_or};
use crate::{ConstTree, Interner, Literal};

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
pub fn slice(s: Binary, i: u64, len: u64) -> XfnResult<Binary> {
  if i + len < s.0.len() as u64 {
    RuntimeError::fail(
      "Byte index out of bounds".to_string(),
      "indexing binary",
    )?
  }
  let data = s.0[i as usize..i as usize + len as usize].to_vec();
  Ok(Binary(Arc::new(data)))
}

/// Return the index where the first argument first contains the second, if any
pub fn find(haystack: Binary, needle: Binary) -> XfnResult<Clause> {
  let found = iter_find(haystack.0.iter(), needle.0.iter());
  Ok(opt(found.map(|x| Literal::Uint(x as u64).into())))
}

/// Split binary data block into two smaller blocks
pub fn split(bin: Binary, i: u64) -> XfnResult<Clause> {
  if bin.0.len() < i as usize {
    RuntimeError::fail(
      "Byte index out of bounds".to_string(),
      "splitting binary",
    )?
  }
  let (asl, bsl) = bin.0.split_at(i as usize);
  Ok(tuple([
    Binary(Arc::new(asl.to_vec())).atom_cls().into(),
    Binary(Arc::new(bsl.to_vec())).atom_cls().into(),
  ]))
}

/// Read a number from a binary blob
pub fn get_num(
  buf: Binary,
  loc: u64,
  size: u64,
  is_le: Boolean,
) -> XfnResult<Literal> {
  if buf.0.len() < (loc + size) as usize {
    RuntimeError::fail(
      "section out of range".to_string(),
      "reading number from binary data",
    )?
  }
  if 8 < size {
    RuntimeError::fail(
      "more than 8 bytes provided".to_string(),
      "reading number from binary data",
    )?
  }
  let mut data = [0u8; 8];
  let section = &buf.0[loc as usize..(loc + size) as usize];
  let num = if is_le.0 {
    data[0..size as usize].copy_from_slice(section);
    u64::from_le_bytes(data)
  } else {
    data[8 - size as usize..].copy_from_slice(section);
    u64::from_be_bytes(data)
  };
  Ok(Literal::Uint(num))
}

/// Convert a number into a blob
pub fn from_num(size: u64, is_le: Boolean, data: u64) -> XfnResult<Binary> {
  if size > 8 {
    RuntimeError::fail(
      "more than 8 bytes requested".to_string(),
      "converting number to binary",
    )?
  }
  let bytes = if is_le.0 {
    data.to_le_bytes()[0..size as usize].to_vec()
  } else {
    data.to_be_bytes()[8 - size as usize..].to_vec()
  };
  Ok(Binary(Arc::new(bytes)))
}

/// Detect the number of bytes in the blob
pub fn size(b: Binary) -> XfnResult<Literal> {
  Ok(Literal::Uint(b.0.len() as u64))
}

pub fn bin(i: &Interner) -> ConstTree {
  ConstTree::tree([(
    i.i("bin"),
    ConstTree::tree([
      (i.i("concat"), ConstTree::xfn(xfn_2ary(concatenate))),
      (i.i("slice"), ConstTree::xfn(xfn_3ary(slice))),
      (i.i("find"), ConstTree::xfn(xfn_2ary(find))),
      (i.i("split"), ConstTree::xfn(xfn_2ary(split))),
      (i.i("get_num"), ConstTree::xfn(xfn_4ary(get_num))),
      (i.i("from_num"), ConstTree::xfn(xfn_3ary(from_num))),
      (i.i("size"), ConstTree::xfn(xfn_1ary(size))),
    ]),
  )])
}
