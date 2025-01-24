use std::cell::RefCell;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::thread;

use async_process::{self, Child, ChildStdin, ChildStdout};
use async_std::io::{self, BufReadExt, BufReader};
use async_std::sync::Mutex;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use futures::task::LocalSpawnExt;
use orchid_api_traits::{Decode, Encode};
use orchid_base::builtin::{ExtInit, ExtPort};
use orchid_base::logging::Logger;
use orchid_base::msg::{recv_msg, send_msg};

use crate::api;
use crate::ctx::Ctx;

pub async fn ext_command(
	cmd: std::process::Command,
	logger: Logger,
	ctx: Ctx,
) -> io::Result<ExtInit> {
	let prog_pbuf = PathBuf::from(cmd.get_program());
	let prog = prog_pbuf.file_stem().unwrap_or(cmd.get_program()).to_string_lossy().to_string();
	let mut child = async_process::Command::from(cmd)
		.stdin(async_process::Stdio::piped())
		.stdout(async_process::Stdio::piped())
		.stderr(async_process::Stdio::piped())
		.spawn()?;
	let mut stdin = child.stdin.take().unwrap();
	api::HostHeader { log_strategy: logger.strat() }.encode(Pin::new(&mut stdin));
	let mut stdout = child.stdout.take().unwrap();
	let header = api::ExtensionHeader::decode(Pin::new(&mut stdout)).await;
	let child_stderr = child.stderr.take().unwrap();
	thread::Builder::new().name(format!("stderr-fwd:{prog}")).spawn(move || {
		async_std::task::block_on(async move {
			let mut reader = BufReader::new(child_stderr);
			loop {
				let mut buf = String::new();
				if 0 == reader.read_line(&mut buf).await.unwrap() {
					break;
				}
				logger.log(buf);
			}
		})
	})?;
	Ok(ExtInit {
		header,
		port: Box::new(Subprocess {
			child: Rc::new(RefCell::new(child)),
			stdin: Mutex::new(Box::pin(stdin)),
			stdout: Mutex::new(Box::pin(stdout)),
			ctx,
		}),
	})
}

pub struct Subprocess {
	child: Rc<RefCell<Child>>,
	stdin: Mutex<Pin<Box<ChildStdin>>>,
	stdout: Mutex<Pin<Box<ChildStdout>>>,
	ctx: Ctx,
}
impl Drop for Subprocess {
	fn drop(&mut self) {
		let child = self.child.clone();
		(self.ctx.spawn.spawn_local(async move {
			let status = child.borrow_mut().status().await.expect("Extension exited with error");
			assert!(status.success(), "Extension exited with error {status}");
		}))
		.expect("Could not spawn process terminating future")
	}
}
impl ExtPort for Subprocess {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()> {
		if msg.starts_with(&[0, 0, 0, 0x1c]) {
			panic!("Received unnecessary prefix");
		}
		async { send_msg(Pin::new(&mut *self.stdin.lock().await), msg).await.unwrap() }.boxed_local()
	}
	fn recv<'a>(
		&'a self,
		cb: Box<dyn FnOnce(&[u8]) -> LocalBoxFuture<'_, ()> + 'a>,
	) -> LocalBoxFuture<'a, ()> {
		Box::pin(async {
			match recv_msg(self.stdout.lock().await.as_mut()).await {
				Ok(msg) => cb(&msg).await,
				Err(e) if e.kind() == io::ErrorKind::BrokenPipe => (),
				Err(e) => panic!("Failed to read from stdout: {}, {e}", e.kind()),
			}
		})
	}
}
