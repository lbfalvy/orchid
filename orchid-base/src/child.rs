use std::io::{self, Read, Write};
use std::sync::Mutex;
use std::{mem, process};

pub struct SharedChild {
  child: process::Child,
  stdin: Mutex<process::ChildStdin>,
  stdout: Mutex<process::ChildStdout>,
}
impl SharedChild {
  pub fn new(cmd: &mut process::Command) -> io::Result<Self> {
    let mut child = cmd.stdin(process::Stdio::piped()).stdout(process::Stdio::piped()).spawn()?;
    let stdin = Mutex::new(child.stdin.take().expect("Piped stdin above"));
    let stdout = Mutex::new(child.stdout.take().expect("Piped stdout above"));
    Ok(Self { stdin, stdout, child })
  }

  pub fn send_msg(&self, msg: &[u8]) -> io::Result<()> {
    send_msg(&mut *self.stdin.lock().unwrap(), msg)
  }

  pub fn recv_msg(&self) -> io::Result<Vec<u8>> { recv_msg(&mut *self.stdout.lock().unwrap()) }
}
impl Drop for SharedChild {
  fn drop(&mut self) { mem::drop(self.child.kill()) }
}

pub fn send_msg(write: &mut impl Write, msg: &[u8]) -> io::Result<()> {
  write.write_all(&(u32::try_from(msg.len()).unwrap()).to_be_bytes())?;
  write.write_all(msg)?;
  write.flush()
}

pub fn recv_msg(read: &mut impl Read) -> io::Result<Vec<u8>> {
  let mut len = [0u8; 4];
  read.read_exact(&mut len)?;
  let len = u32::from_be_bytes(len);
  let mut msg = vec![0u8; len as usize];
  read.read_exact(&mut msg)?;
  Ok(msg)
}

pub fn send_parent_msg(msg: &[u8]) -> io::Result<()> { send_msg(&mut io::stdout().lock(), msg) }
pub fn recv_parent_msg() -> io::Result<Vec<u8>> { recv_msg(&mut io::stdin().lock()) }
