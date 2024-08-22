use std::io::{self, BufRead as _};
use std::path::PathBuf;
use std::sync::Mutex;
use std::{process, thread};

use orchid_base::logging::Logger;
use orchid_base::msg::{recv_msg, send_msg};

use crate::extension::ExtensionPort;

pub struct Subprocess {
  child: Mutex<process::Child>,
  stdin: Mutex<process::ChildStdin>,
  stdout: Mutex<process::ChildStdout>,
}
impl Subprocess {
  pub fn new(mut cmd: process::Command, logger: Logger) -> io::Result<Self> {
    let prog_pbuf = PathBuf::from(cmd.get_program());
    let prog = prog_pbuf.file_stem().unwrap_or(cmd.get_program()).to_string_lossy().to_string();
    let mut child = cmd
      .stdin(process::Stdio::piped())
      .stdout(process::Stdio::piped())
      .stderr(process::Stdio::piped())
      .spawn()?;
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let child_stderr = child.stderr.take().unwrap();
    thread::Builder::new().name(format!("stderr-fwd:{prog}")).spawn(move || {
      let mut reader = io::BufReader::new(child_stderr);
      loop {
        let mut buf = String::new();
        if 0 == reader.read_line(&mut buf).unwrap() {
          break;
        }
        logger.log(buf);
      }
    })?;
    Ok(Self { child: Mutex::new(child), stdin: Mutex::new(stdin), stdout: Mutex::new(stdout) })
  }
}
impl Drop for Subprocess {
  fn drop(&mut self) { self.child.lock().unwrap().wait().expect("Extension exited with error"); }
}
impl ExtensionPort for Subprocess {
  fn send(&self, msg: &[u8]) { send_msg(&mut *self.stdin.lock().unwrap(), msg).unwrap() }
  fn receive(&self) -> Option<Vec<u8>> {
    match recv_msg(&mut *self.stdout.lock().unwrap()) {
      Ok(msg) => Some(msg),
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => None,
      Err(e) => panic!("Failed to read from stdout: {}, {e}", e.kind()),
    }
  }
}
