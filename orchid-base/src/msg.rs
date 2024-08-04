use orchid_api_traits::Decode;
use std::io;

use orchid_api_traits::Encode;

pub fn send_msg(write: &mut impl io::Write, msg: &[u8]) -> io::Result<()> {
  u32::try_from(msg.len()).unwrap().encode(write);
  write.write_all(msg)?;
  write.flush()
}

pub fn recv_msg(read: &mut impl io::Read) -> io::Result<Vec<u8>> {
  let len = u32::decode(read);
  let mut msg = vec![0u8; len as usize];
  read.read_exact(&mut msg)?;
  Ok(msg)
}
