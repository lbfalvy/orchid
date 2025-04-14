use crate::entrypoint::ExtensionData;

#[cfg(feature = "tokio")]
pub async fn tokio_main(data: ExtensionData) {
	use std::io::Write;
	use std::mem;
	use std::pin::Pin;
	use std::rc::Rc;

	use async_std::io;
	use futures::StreamExt;
	use futures::future::LocalBoxFuture;
	use futures::stream::FuturesUnordered;
	use orchid_api_traits::{Decode, Encode};
	use tokio::task::{LocalSet, spawn_local};

	use crate::api;
	use crate::entrypoint::extension_init;
	use crate::msg::{recv_parent_msg, send_parent_msg};

	let local_set = LocalSet::new();
	local_set.spawn_local(async {
		let host_header = api::HostHeader::decode(Pin::new(&mut async_std::io::stdin())).await;
		let init =
			Rc::new(extension_init(data, host_header, Rc::new(|fut| mem::drop(spawn_local(fut)))));
		let mut buf = Vec::new();
		init.header.encode(Pin::new(&mut buf)).await;
		std::io::stdout().write_all(&buf).unwrap();
		std::io::stdout().flush().unwrap();
		// These are concurrent processes that never exit, so if the FuturesUnordered
		// produces any result the extension should exit
		let mut io = FuturesUnordered::<LocalBoxFuture<()>>::new();
		io.push(Box::pin(async {
			loop {
				match recv_parent_msg().await {
					Ok(msg) => init.send(&msg[..]).await,
					Err(e) if e.kind() == io::ErrorKind::BrokenPipe => break,
					Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
					Err(e) => panic!("{e}"),
				}
			}
		}));
		io.push(Box::pin(async {
			while let Some(msg) = init.recv().await {
				send_parent_msg(&msg[..]).await.unwrap();
			}
		}));
		io.next().await;
	});
	local_set.await;
}
