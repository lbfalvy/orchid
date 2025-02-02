use std::ops::Deref;
use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::api;

pub type Spawner = Rc<dyn Fn(LocalBoxFuture<'static, ()>)>;
pub type RecvCB<'a> = Box<dyn for<'b> FnOnce(&'b [u8]) -> LocalBoxFuture<'b, ()> + 'a>;

/// The 3 primary contact points with an extension are
/// - send a message
/// - wait for a message to arrive
/// - wait for the extension to stop after exit (this is the implicit Drop)
///
/// There are no ordering guarantees about these
pub trait ExtPort {
	fn send<'a>(&'a self, msg: &'a [u8]) -> LocalBoxFuture<'a, ()>;
	fn recv<'a>(&'a self, cb: RecvCB<'a>) -> LocalBoxFuture<'a, ()>;
}

pub struct ExtInit {
	pub header: api::ExtensionHeader,
	pub port: Box<dyn ExtPort>,
}
impl ExtInit {
	pub async fn send(&self, msg: &[u8]) { self.port.send(msg).await }
	pub async fn recv<'a, 's: 'a>(&'s self, cb: RecvCB<'a>) { self.port.recv(Box::new(cb)).await }
}
impl Deref for ExtInit {
	type Target = api::ExtensionHeader;
	fn deref(&self) -> &Self::Target { &self.header }
}
