use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::ops::{BitAnd, Deref};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_std::channel;
use async_std::sync::Mutex;
use derive_destructure::destructure;
use dyn_clone::{DynClone, clone_box};
use futures::future::LocalBoxFuture;
use hashbrown::HashMap;
use orchid_api_traits::{Channel, Coding, Decode, Encode, MsgSet, Request};
use trait_set::trait_set;

use crate::clone;

pub struct Receipt<'a>(PhantomData<&'a mut ()>);

trait_set! {
	pub trait SendFn<T: MsgSet> =
		for<'a> FnMut(&'a [u8], ReqNot<T>) -> LocalBoxFuture<'a, ()>
		+ DynClone + 'static;
	pub trait ReqFn<T: MsgSet> =
		for<'a> FnMut(RequestHandle<'a, T>, <T::In as Channel>::Req)
			-> LocalBoxFuture<'a, Receipt<'a>>
		+ DynClone + 'static;
	pub trait NotifFn<T: MsgSet> =
		FnMut(<T::In as Channel>::Notif, ReqNot<T>) -> LocalBoxFuture<'static, ()>
		+ DynClone + 'static;
}

fn get_id(message: &[u8]) -> (u64, &[u8]) {
	(u64::from_be_bytes(message[..8].to_vec().try_into().unwrap()), &message[8..])
}

pub trait ReqHandlish {
	fn defer_drop(&self, val: impl Any + 'static)
	where Self: Sized {
		self.defer_drop_objsafe(Box::new(val));
	}
	fn defer_drop_objsafe(&self, val: Box<dyn Any>);
}

#[derive(destructure)]
pub struct RequestHandle<'a, MS: MsgSet> {
	defer_drop: RefCell<Vec<Box<dyn Any>>>,
	fulfilled: AtomicBool,
	id: u64,
	_reqlt: PhantomData<&'a mut ()>,
	parent: ReqNot<MS>,
}
impl<'a, MS: MsgSet + 'static> RequestHandle<'a, MS> {
	fn new(parent: ReqNot<MS>, id: u64) -> Self {
		Self {
			defer_drop: RefCell::default(),
			fulfilled: false.into(),
			_reqlt: PhantomData,
			parent,
			id,
		}
	}
	pub fn reqnot(&self) -> ReqNot<MS> { self.parent.clone() }
	pub async fn handle<U: Request>(&self, _: &U, rep: &U::Response) -> Receipt<'a> {
		self.respond(rep).await
	}
	pub fn will_handle_as<U: Request>(&self, _: &U) -> ReqTypToken<U> { ReqTypToken(PhantomData) }
	pub async fn handle_as<U: Request>(&self, _: ReqTypToken<U>, rep: &U::Response) -> Receipt<'a> {
		self.respond(rep).await
	}
	pub async fn respond(&self, response: &impl Encode) -> Receipt<'a> {
		assert!(!self.fulfilled.swap(true, Ordering::Relaxed), "Already responded to {}", self.id);
		let mut buf = (!self.id).to_be_bytes().to_vec();
		response.encode(Pin::new(&mut buf)).await;
		let mut send = clone_box(&*self.reqnot().0.lock().await.send);
		(send)(&buf, self.parent.clone()).await;
		Receipt(PhantomData)
	}
}
impl<MS: MsgSet> ReqHandlish for RequestHandle<'_, MS> {
	fn defer_drop_objsafe(&self, val: Box<dyn Any>) { self.defer_drop.borrow_mut().push(val); }
}
impl<MS: MsgSet> Drop for RequestHandle<'_, MS> {
	fn drop(&mut self) {
		let done = self.fulfilled.load(Ordering::Relaxed);
		debug_assert!(done, "Request {} dropped without response", self.id)
	}
}

pub struct ReqTypToken<T>(PhantomData<T>);

pub struct ReqNotData<T: MsgSet> {
	id: u64,
	send: Box<dyn SendFn<T>>,
	notif: Box<dyn NotifFn<T>>,
	req: Box<dyn ReqFn<T>>,
	responses: HashMap<u64, channel::Sender<Vec<u8>>>,
}

/// Wraps a raw message buffer to save on copying.
/// Dereferences to the tail of the message buffer, cutting off the ID
#[derive(Debug, Clone)]
pub struct RawReply(Vec<u8>);
impl Deref for RawReply {
	type Target = [u8];
	fn deref(&self) -> &Self::Target { get_id(&self.0[..]).1 }
}

pub struct ReqNot<T: MsgSet>(Arc<Mutex<ReqNotData<T>>>);
impl<T: MsgSet> ReqNot<T> {
	pub fn new(send: impl SendFn<T>, notif: impl NotifFn<T>, req: impl ReqFn<T>) -> Self {
		Self(Arc::new(Mutex::new(ReqNotData {
			id: 1,
			send: Box::new(send),
			notif: Box::new(notif),
			req: Box::new(req),
			responses: HashMap::new(),
		})))
	}

	/// Can be called from a polling thread or dispatched in any other way
	pub async fn receive(&self, message: &[u8]) {
		let mut g = self.0.lock().await;
		let (id, payload) = get_id(message);
		if id == 0 {
			let mut notif_cb = clone_box(&*g.notif);
			mem::drop(g);
			let notif_val = <T::In as Channel>::Notif::decode(Pin::new(&mut &payload[..])).await;
			notif_cb(notif_val, self.clone()).await
		} else if 0 < id.bitand(1 << 63) {
			let sender = g.responses.remove(&!id).expect("Received response for invalid message");
			sender.send(message.to_vec()).await.unwrap();
		} else {
			let message = <T::In as Channel>::Req::decode(Pin::new(&mut &payload[..])).await;
			let mut req_cb = clone_box(&*g.req);
			mem::drop(g);
			let rn = self.clone();
			req_cb(RequestHandle::new(rn, id), message).await;
		}
	}

	pub async fn notify<N: Coding + Into<<T::Out as Channel>::Notif>>(&self, notif: N) {
		let mut send = clone_box(&*self.0.lock().await.send);
		let mut buf = vec![0; 8];
		let msg: <T::Out as Channel>::Notif = notif.into();
		msg.encode(Pin::new(&mut buf)).await;
		send(&buf, self.clone()).await
	}
}

pub trait DynRequester {
	type Transfer;
	/// Encode and send a request, then receive the response buffer.
	fn raw_request(&self, data: Self::Transfer) -> LocalBoxFuture<'_, RawReply>;
}

pub struct MappedRequester<'a, T: 'a>(Box<dyn Fn(T) -> LocalBoxFuture<'a, RawReply> + 'a>);
impl<'a, T> MappedRequester<'a, T> {
	fn new<U: DynRequester + 'a>(req: U) -> Self
	where T: Into<U::Transfer> {
		let req_arc = Arc::new(req);
		MappedRequester(Box::new(move |t| {
			Box::pin(clone!(req_arc; async move { req_arc.raw_request(t.into()).await}))
		}))
	}
}

impl<T> DynRequester for MappedRequester<'_, T> {
	type Transfer = T;
	fn raw_request(&self, data: Self::Transfer) -> LocalBoxFuture<'_, RawReply> { self.0(data) }
}

impl<T: MsgSet> DynRequester for ReqNot<T> {
	type Transfer = <T::Out as Channel>::Req;
	fn raw_request(&self, req: Self::Transfer) -> LocalBoxFuture<'_, RawReply> {
		Box::pin(async move {
			let mut g = self.0.lock().await;
			let id = g.id;
			g.id += 1;
			let mut buf = id.to_be_bytes().to_vec();
			req.encode(Pin::new(&mut buf)).await;
			let (send, recv) = channel::bounded(1);
			g.responses.insert(id, send);
			let mut send = clone_box(&*g.send);
			mem::drop(g);
			let rn = self.clone();
			send(&buf, rn).await;
			RawReply(recv.recv().await.unwrap())
		})
	}
}

pub trait Requester: DynRequester {
	#[must_use = "These types are subject to change with protocol versions. \
    If you don't want to use the return value, At a minimum, force the type."]
	fn request<R: Request + Into<Self::Transfer>>(
		&self,
		data: R,
	) -> impl Future<Output = R::Response>;
	fn map<'a, U: Into<Self::Transfer>>(self) -> MappedRequester<'a, U>
	where Self: Sized + 'a {
		MappedRequester::new(self)
	}
}
impl<This: DynRequester + ?Sized> Requester for This {
	async fn request<R: Request + Into<Self::Transfer>>(&self, data: R) -> R::Response {
		R::Response::decode(Pin::new(&mut &self.raw_request(data.into()).await[..])).await
	}
}

impl<T: MsgSet> Clone for ReqNot<T> {
	fn clone(&self) -> Self { Self(self.0.clone()) }
}

#[cfg(test)]
mod test {
	use std::rc::Rc;
	use std::sync::Arc;

	use async_std::sync::Mutex;
	use futures::FutureExt;
	use orchid_api_derive::Coding;
	use orchid_api_traits::{Channel, Request};
	use test_executors::spin_on;

	use super::{MsgSet, ReqNot};
	use crate::clone;
	use crate::reqnot::Requester as _;

	#[derive(Clone, Debug, Coding, PartialEq)]
	pub struct TestReq(u8);
	impl Request for TestReq {
		type Response = u8;
	}

	pub struct TestChan;
	impl Channel for TestChan {
		type Notif = u8;
		type Req = TestReq;
	}

	pub struct TestMsgSet;
	impl MsgSet for TestMsgSet {
		type In = TestChan;
		type Out = TestChan;
	}

	#[test]
	fn notification() {
		spin_on(async {
			let received = Arc::new(Mutex::new(None));
			let receiver = ReqNot::<TestMsgSet>::new(
				|_, _| panic!("Should not send anything"),
				clone!(received; move |notif, _| clone!(received; async move {
					*received.lock().await = Some(notif);
				}.boxed_local())),
				|_, _| panic!("Not receiving a request"),
			);
			let sender = ReqNot::<TestMsgSet>::new(
				clone!(receiver; move |d, _| clone!(receiver; Box::pin(async move {
					receiver.receive(d).await
				}))),
				|_, _| panic!("Should not receive notif"),
				|_, _| panic!("Should not receive request"),
			);
			sender.notify(3).await;
			assert_eq!(*received.lock().await, Some(3));
			sender.notify(4).await;
			assert_eq!(*received.lock().await, Some(4));
		})
	}

	#[test]
	fn request() {
		spin_on(async {
			let receiver = Rc::new(Mutex::<Option<ReqNot<TestMsgSet>>>::new(None));
			let sender = Rc::new(ReqNot::<TestMsgSet>::new(
				clone!(receiver; move |d, _| clone!(receiver; Box::pin(async move {
					receiver.lock().await.as_ref().unwrap().receive(d).await
				}))),
				|_, _| panic!("Should not receive notif"),
				|_, _| panic!("Should not receive request"),
			));
			*receiver.lock().await = Some(ReqNot::new(
				clone!(sender; move |d, _| clone!(sender; Box::pin(async move {
					sender.receive(d).await
				}))),
				|_, _| panic!("Not receiving notifs"),
				|hand, req| {
					Box::pin(async move {
						assert_eq!(req, TestReq(5));
						hand.respond(&6u8).await
					})
				},
			));
			let response = sender.request(TestReq(5)).await;
			assert_eq!(response, 6);
		})
	}
}
