use std::io;
use std::sync::Mutex;
use std::{mem, process};

use orchid_base::msg::{recv_msg, send_msg};

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
