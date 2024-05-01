use std::io;

pub fn send_msg(write: &mut impl io::Write, msg: &[u8]) -> io::Result<()> {
  write.write_all(&(u32::try_from(msg.len()).unwrap()).to_be_bytes())?;
  write.write_all(msg)?;
  write.flush()
}

pub fn recv_msg(read: &mut impl io::Read) -> io::Result<Vec<u8>> {
  let mut len = [0u8; 4];
  read.read_exact(&mut len)?;
  let len = u32::from_be_bytes(len);
  let mut msg = vec![0u8; len as usize];
  read.read_exact(&mut msg)?;
  Ok(msg)
}
