use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;

use crate::api;

/// The 3 primary contact points with an extension are
/// - send a message
/// - wait for a message to arrive
/// - wait for the extension to stop after exit (this is the implicit Drop)
///
/// There are no ordering guarantees about these
pub trait ExtPort {
	fn send(&self, msg: &[u8]) -> Pin<Box<dyn Future<Output = ()>>>;
	fn recv<'a>(&self, cb: Box<dyn FnOnce(&[u8]) + Send + 'a>) -> Pin<Box<dyn Future<Output = ()>>>;
}

pub struct ExtInit {
	pub header: api::ExtensionHeader,
	pub port: Box<dyn ExtPort>,
}
impl ExtInit {
	pub async fn send(&self, msg: &[u8]) { self.port.send(msg).await }
	pub async fn recv(&self, cb: impl FnOnce(&[u8]) + Send) { self.port.recv(Box::new(cb)).await }
}
impl Deref for ExtInit {
	type Target = api::ExtensionHeader;
	fn deref(&self) -> &Self::Target { &self.header }
}
