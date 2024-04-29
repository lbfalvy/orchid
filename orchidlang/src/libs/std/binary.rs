//! `std::binary` Operations on binary buffers.

use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use itertools::Itertools;

use super::runtime_error::RuntimeError;
use crate::foreign::atom::Atomic;
use crate::foreign::error::RTResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::gen::tree::{atom_ent, xfn_ent, ConstTree};
use crate::interpreter::nort::Clause;
use crate::utils::iter_find::iter_find;
use crate::utils::unwrap_or::unwrap_or;

const INT_BYTES: usize = usize::BITS as usize / 8;

/// A block of binary data
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct Binary(pub Arc<Vec<u8>>);
impl InertPayload for Binary {
  const TYPE_STR: &'static str = "a binary blob";
}

impl Deref for Binary {
  type Target = Vec<u8>;
  fn deref(&self) -> &Self::Target { &self.0 }
}

impl fmt::Debug for Binary {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut iter = self.0.iter().copied();
    f.write_str("Binary")?;
    for mut chunk in iter.by_ref().take(32).chunks(4).into_iter() {
      let a = chunk.next().expect("Chunks cannot be empty");
      let b = unwrap_or!(chunk.next(); return write!(f, "{a:02x}"));
      let c = unwrap_or!(chunk.next(); return write!(f, "{a:02x}{b:02x}"));
      let d = unwrap_or!(chunk.next(); return write!(f, "{a:02x}{b:02x}{c:02x}"));
      write!(f, "{a:02x}{b:02x}{c:02x}{d:02x}")?
    }
    if iter.next().is_some() { write!(f, "...") } else { Ok(()) }
  }
}

/// Append two binary data blocks
pub fn concatenate(a: Inert<Binary>, b: Inert<Binary>) -> Inert<Binary> {
  let data = (*a).iter().chain(b.0.0.iter()).copied().collect();
  Inert(Binary(Arc::new(data)))
}

/// Extract a subsection of the binary data
pub fn slice(s: Inert<Binary>, i: Inert<usize>, len: Inert<usize>) -> RTResult<Inert<Binary>> {
  if i.0 + len.0 < s.0.0.len() {
    RuntimeError::fail("Byte index out of bounds".to_string(), "indexing binary")?
  }
  Ok(Inert(Binary(Arc::new(s.0.0[i.0..i.0 + len.0].to_vec()))))
}

/// Return the index where the first argument first contains the second, if any
pub fn find(haystack: Inert<Binary>, needle: Inert<Binary>) -> Option<Clause> {
  let found = iter_find(haystack.0.0.iter(), needle.0.0.iter());
  found.map(|i| Inert(i).atom_cls())
}

/// Split binary data block into two smaller blocks
pub fn split(bin: Inert<Binary>, i: Inert<usize>) -> RTResult<(Inert<Binary>, Inert<Binary>)> {
  if bin.0.0.len() < i.0 {
    RuntimeError::fail("Byte index out of bounds".to_string(), "splitting binary")?
  }
  let (asl, bsl) = bin.0.0.split_at(i.0);
  Ok((Inert(Binary(Arc::new(asl.to_vec()))), Inert(Binary(Arc::new(bsl.to_vec())))))
}

/// Read a number from a binary blob
pub fn get_num(
  buf: Inert<Binary>,
  loc: Inert<usize>,
  size: Inert<usize>,
  is_le: Inert<bool>,
) -> RTResult<Inert<usize>> {
  if buf.0.0.len() < (loc.0 + size.0) {
    RuntimeError::fail("section out of range".to_string(), "reading number from binary data")?
  }
  if INT_BYTES < size.0 {
    RuntimeError::fail(
      "more than std::bin::int_bytes bytes provided".to_string(),
      "reading number from binary data",
    )?
  }
  let mut data = [0u8; INT_BYTES];
  let section = &buf.0.0[loc.0..(loc.0 + size.0)];
  let num = if is_le.0 {
    data[0..size.0].copy_from_slice(section);
    usize::from_le_bytes(data)
  } else {
    data[INT_BYTES - size.0..].copy_from_slice(section);
    usize::from_be_bytes(data)
  };
  Ok(Inert(num))
}

/// Convert a number into a blob
pub fn from_num(
  size: Inert<usize>,
  is_le: Inert<bool>,
  data: Inert<usize>,
) -> RTResult<Inert<Binary>> {
  if INT_BYTES < size.0 {
    RuntimeError::fail(
      "more than std::bin::int_bytes bytes requested".to_string(),
      "converting number to binary",
    )?
  }
  let bytes = match is_le.0 {
    true => data.0.to_le_bytes()[0..size.0].to_vec(),
    false => data.0.to_be_bytes()[8 - size.0..].to_vec(),
  };
  Ok(Inert(Binary(Arc::new(bytes))))
}

/// Detect the number of bytes in the blob
pub fn size(b: Inert<Binary>) -> Inert<usize> { Inert(b.0.len()) }

pub(super) fn bin_lib() -> ConstTree {
  ConstTree::ns("std::binary", [ConstTree::tree([
    xfn_ent("concat", [concatenate]),
    xfn_ent("slice", [slice]),
    xfn_ent("find", [find]),
    xfn_ent("split", [split]),
    xfn_ent("get_num", [get_num]),
    xfn_ent("from_num", [from_num]),
    xfn_ent("size", [size]),
    atom_ent("int_bytes", [Inert(INT_BYTES)]),
  ])])
}
