use std::cell::RefCell;
use std::pin::Pin;

use async_process::{self, Child, ChildStdin, ChildStdout};
use async_std::io::{self, BufReadExt, BufReader};
use async_std::sync::Mutex;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use orchid_api_traits::{Decode, Encode};
use orchid_base::builtin::{ExtInit, ExtPort};
use orchid_base::logging::Logger;
use orchid_base::msg::{recv_msg, send_msg};

use crate::api;
use crate::ctx::Ctx;

pub async fn ext_command(
	cmd: std::process::Command,
	logger: Logger,
	msg_logs: Logger,
	ctx: Ctx,
) -> io::Result<ExtInit> {
	let mut child = async_process::Command::from(cmd)
		.stdin(async_process::Stdio::piped())
		.stdout(async_process::Stdio::piped())
		.stderr(async_process::Stdio::piped())
		.spawn()?;
	let mut stdin = child.stdin.take().unwrap();
	api::HostHeader { log_strategy: logger.strat(), msg_logs: msg_logs.strat() }
		.encode(Pin::new(&mut stdin))
		.await;
	let mut stdout = child.stdout.take().unwrap();
	let header = api::ExtensionHeader::decode(Pin::new(&mut stdout)).await;
	let child_stderr = child.stderr.take().unwrap();
	(ctx.spawn)(Box::pin(async move {
		let mut reader = BufReader::new(child_stderr);
		loop {
			let mut buf = String::new();
			if 0 == reader.read_line(&mut buf).await.unwrap() {
				break;
			}
			logger.log(buf.strip_suffix('\n').expect("Readline implies this"));
		}
	}));
	Ok(ExtInit {
		header,
		port: Box::new(Subprocess {
			child: RefCell::new(Some(child)),
			stdin: Mutex::new(Box::pin(stdin)),
			stdout: Mutex::new(Box::pin(stdout)),
			ctx,
		}),
	})
}

pub struct Subprocess {
	child: RefCell<Option<Child>>,
	stdin: Mutex<Pin<Box<ChildStdin>>>,
	stdout: Mutex<Pin<Box<ChildStdout>>>,
	ctx: Ctx,
}
impl Drop for Subprocess {
	fn drop(&mut self) {
		let mut child = self.child.borrow_mut().take().unwrap();
		(self.ctx.spawn)(Box::pin(async move {
			let status = child.status().await.expect("Extension exited with error");
			assert!(status.success(), "Extension exited with error {status}");
		}))
	}
}
impl ExtPort for Subprocess {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()> {
		async { send_msg(Pin::new(&mut *self.stdin.lock().await), msg).await.unwrap() }.boxed_local()
	}
	fn recv<'a>(
		&'a self,
		cb: Box<dyn FnOnce(&[u8]) -> LocalBoxFuture<'_, ()> + 'a>,
	) -> LocalBoxFuture<'a, ()> {
		Box::pin(async {
			std::io::Write::flush(&mut std::io::stderr()).unwrap();
			match recv_msg(self.stdout.lock().await.as_mut()).await {
				Ok(msg) => cb(&msg).await,
				Err(e) if e.kind() == io::ErrorKind::BrokenPipe => (),
				Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => (),
				Err(e) => panic!("Failed to read from stdout: {}, {e}", e.kind()),
			}
		})
	}
}
