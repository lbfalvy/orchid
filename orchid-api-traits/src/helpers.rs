use std::io::{Read, Write};

use crate::Encode;

pub fn encode_enum<W: Write>(write: &mut W, id: u8, f: impl FnOnce(&mut W)) {
  id.encode(write);
  f(write)
}

pub fn write_exact(write: &mut impl Write, bytes: &'static [u8]) {
  write.write_all(bytes).expect("Failed to write exact bytes")
}

pub fn read_exact(read: &mut impl Read, bytes: &'static [u8]) {
  let mut data = vec![0u8; bytes.len()];
  read.read_exact(&mut data).expect("Failed to read bytes");
  assert_eq!(&data, bytes, "Wrong bytes")
}
