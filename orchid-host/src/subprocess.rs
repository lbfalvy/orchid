use std::io::{self, BufRead as _, Write};
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::{process, thread};

use async_std::sync::Mutex;
use orchid_api_traits::{Decode, Encode};
use orchid_base::builtin::{ExtInit, ExtPort};
use orchid_base::logging::Logger;
use orchid_base::msg::{recv_msg, send_msg};

use crate::api;

pub struct ExtensionCommand(pub process::Command, pub Logger);
impl ExtFactory for ExtensionCommand {
	fn run(self: Box<Self>, onmessage: OnMessage) -> ExtInit {
		let Self(mut cmd, logger) = *self;
		let prog_pbuf = PathBuf::from(cmd.get_program());
		let prog = prog_pbuf.file_stem().unwrap_or(cmd.get_program()).to_string_lossy().to_string();
		let mut child = cmd
			.stdin(process::Stdio::piped())
			.stdout(process::Stdio::piped())
			.stderr(process::Stdio::piped())
			.spawn()?;
		let mut stdin = child.stdin.take().unwrap();
		api::HostHeader { log_strategy: logger.strat() }.encode(&mut stdin);
		stdin.flush()?;
		let mut stdout = child.stdout.take().unwrap();
		let header = api::ExtensionHeader::decode(&mut stdout);
		let child_stderr = child.stderr.take().unwrap();
		let (set_onmessage, recv_onmessage) = sync_channel(0);
		thread::Builder::new().name(format!("stdout-fwd:{prog}")).spawn(move || {
			let mut onmessage: Box<dyn FnMut(&[u8]) + Send> = recv_onmessage.recv().unwrap();
			drop(recv_onmessage);
			loop {
				match recv_msg(&mut stdout) {
					Ok(msg) => onmessage(&msg[..]),
					Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
					Err(e) => panic!("Failed to read from stdout: {}, {e}", e.kind()),
				}
			}
		})?;
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
		Ok(Subprocess { child: Mutex::new(child), stdin: Mutex::new(stdin), set_onmessage, header })
	}
}

pub struct Subprocess {
	child: Mutex<process::Child>,
	stdin: Mutex<process::ChildStdin>,
	stdout: Mutex<process::ChildStdout>,
	header: api::ExtensionHeader,
}
impl Subprocess {
	pub fn new(mut cmd: process::Command, logger: Logger) -> io::Result<Self> {}
}
impl Drop for Subprocess {
	fn drop(&mut self) { self.child.lock().unwrap().wait().expect("Extension exited with error"); }
}
impl ExtPort for Subprocess {
	fn send(&self, msg: &[u8]) {
		if msg.starts_with(&[0, 0, 0, 0x1c]) {
			panic!("Received unnecessary prefix");
		}
		send_msg(&mut *self.stdin.lock().unwrap(), msg).unwrap()
	}
	fn recv<'a>(&self, cb: Box<dyn FnOnce(&[u8]) + Send + 'a>) -> futures::future::BoxFuture<()> {
		async {
      
    }
	}
}
