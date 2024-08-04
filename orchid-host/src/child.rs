use std::sync::Mutex;
use std::{fmt, io, mem, process};

use orchid_base::msg::{recv_msg, send_msg};

pub struct SharedChild {
  child: process::Child,
  stdin: Mutex<process::ChildStdin>,
  stdout: Mutex<process::ChildStdout>,
  debug: Option<(String, Mutex<Box<dyn fmt::Write>>)>,
}
impl SharedChild {
  pub fn new(command: &mut process::Command, debug: Option<(&str, impl fmt::Write + 'static)>) -> io::Result<Self> {
    let mut child = command.stdin(process::Stdio::piped()).stdout(process::Stdio::piped()).spawn()?;
    let stdin = Mutex::new(child.stdin.take().expect("Piped stdin above"));
    let stdout = Mutex::new(child.stdout.take().expect("Piped stdout above"));
    let debug = debug.map(|(n, w)| (n.to_string(), Mutex::new(Box::new(w) as Box<dyn fmt::Write>)));
    Ok(Self { child, stdin, stdout, debug })
  }

  pub fn send_msg(&self, msg: &[u8]) -> io::Result<()> {
    if let Some((n, dbg)) = &self.debug {
      let mut dbg = dbg.lock().unwrap();
      writeln!(dbg, "To {n}: {msg:?}").unwrap();
    }
    send_msg(&mut *self.stdin.lock().unwrap(), msg)
  }

  pub fn recv_msg(&self) -> io::Result<Vec<u8>> {
    let msg = recv_msg(&mut *self.stdout.lock().unwrap());
    if let Some((n, dbg)) = &self.debug {
      let mut dbg = dbg.lock().unwrap();
      writeln!(dbg, "From {n}: {msg:?}").unwrap();
    }
    msg
  }
}
impl Drop for SharedChild {
  fn drop(&mut self) { mem::drop(self.child.kill()) }
}
