use std::ops::Deref;
use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::api;

pub type Spawner = Rc<dyn Fn(LocalBoxFuture<'static, ()>)>;

/// The 3 primary contact points with an extension are
/// - send a message
/// - wait for a message to arrive
/// - wait for the extension to stop after exit (this is the implicit Drop)
///
/// There are no ordering guarantees about these
pub trait ExtPort {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()>;
	fn recv(&self) -> LocalBoxFuture<'_, Option<Vec<u8>>>;
}

pub struct ExtInit {
	pub header: api::ExtensionHeader,
	pub port: Box<dyn ExtPort>,
}
impl ExtInit {
	pub async fn send(&self, msg: &[u8]) { self.port.send(msg).await }
	pub async fn recv(&self) -> Option<Vec<u8>> { self.port.recv().await }
}
impl Deref for ExtInit {
	type Target = api::ExtensionHeader;
	fn deref(&self) -> &Self::Target { &self.header }
}
