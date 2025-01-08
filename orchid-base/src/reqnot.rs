use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{BitAnd, Deref};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::{mem, thread};

use derive_destructure::destructure;
use dyn_clone::{clone_box, DynClone};
use hashbrown::HashMap;
use orchid_api_traits::{Channel, Coding, Decode, Encode, MsgSet, Request};
use trait_set::trait_set;

pub struct Receipt;
impl Receipt {
  pub fn off_thread(name: String, cb: impl FnOnce() -> Self + Send + 'static) -> Self {
    thread::Builder::new().name(name).spawn(cb).unwrap();
    Self
  }
}

trait_set! {
  pub trait SendFn<T: MsgSet> = for<'a> FnMut(&'a [u8], ReqNot<T>) + DynClone + Send + 'static;
  pub trait ReqFn<T: MsgSet> =
    FnMut(RequestHandle<T>, <T::In as Channel>::Req) -> Receipt + DynClone + Send + Sync + 'static;
  pub trait NotifFn<T: MsgSet> =
    for<'a> FnMut(<T::In as Channel>::Notif, ReqNot<T>) + DynClone + Send + Sync + 'static;
}

fn get_id(message: &[u8]) -> (u64, &[u8]) {
  (u64::from_be_bytes(message[..8].to_vec().try_into().unwrap()), &message[8..])
}

pub trait ReqHandlish {
  fn defer_drop(&self, val: impl Any + 'static);
}

#[derive(destructure)]
pub struct RequestHandle<MS: MsgSet> {
  defer_drop: RefCell<Vec<Box<dyn Any>>>,
  fulfilled: AtomicBool,
  id: u64,
  parent: ReqNot<MS>,
}
impl<MS: MsgSet + 'static> RequestHandle<MS> {
  fn new(parent: ReqNot<MS>, id: u64) -> Self {
    Self { defer_drop: RefCell::default(), fulfilled: false.into(), parent, id }
  }
  pub fn reqnot(&self) -> ReqNot<MS> { self.parent.clone() }
  pub fn handle<U: Request>(&self, _: &U, rep: &U::Response) -> Receipt { self.respond(rep) }
  pub fn will_handle_as<U: Request>(&self, _: &U) -> ReqTypToken<U> { ReqTypToken(PhantomData) }
  pub fn handle_as<U: Request>(&self, _: ReqTypToken<U>, rep: &U::Response) -> Receipt {
    self.respond(rep)
  }
  pub fn respond(&self, response: &impl Encode) -> Receipt {
    assert!(!self.fulfilled.swap(true, Ordering::Relaxed), "Already responded to {}", self.id);
    let mut buf = (!self.id).to_be_bytes().to_vec();
    response.encode(&mut buf);
    let mut send = clone_box(&*self.reqnot().0.lock().unwrap().send);
    (send)(&buf, self.parent.clone());
    Receipt
  }
}
impl<MS: MsgSet> ReqHandlish for RequestHandle<MS> {
  fn defer_drop(&self, val: impl Any) { self.defer_drop.borrow_mut().push(Box::new(val)) }
}
impl<MS: MsgSet> Drop for RequestHandle<MS> {
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
  responses: HashMap<u64, SyncSender<Vec<u8>>>,
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
  pub fn receive(&self, message: &[u8]) {
    let mut g = self.0.lock().unwrap();
    let (id, payload) = get_id(message);
    if id == 0 {
      let mut notif = clone_box(&*g.notif);
      mem::drop(g);
      notif(<T::In as Channel>::Notif::decode(&mut &payload[..]), self.clone())
    } else if 0 < id.bitand(1 << 63) {
      let sender = g.responses.remove(&!id).expect("Received response for invalid message");
      sender.send(message.to_vec()).unwrap();
    } else {
      let message = <T::In as Channel>::Req::decode(&mut &payload[..]);
      let mut req = clone_box(&*g.req);
      mem::drop(g);
      let rn = self.clone();
      thread::Builder::new()
        .name(format!("request {id}"))
        .spawn(move || req(RequestHandle::new(rn, id), message))
        .unwrap();
    }
  }

  pub fn notify<N: Coding + Into<<T::Out as Channel>::Notif>>(&self, notif: N) {
    let mut send = clone_box(&*self.0.lock().unwrap().send);
    let mut buf = vec![0; 8];
    let msg: <T::Out as Channel>::Notif = notif.into();
    msg.encode(&mut buf);
    send(&buf, self.clone())
  }
}

pub trait DynRequester: Send + Sync {
  type Transfer;
  /// Encode and send a request, then receive the response buffer.
  fn raw_request(&self, data: Self::Transfer) -> RawReply;
}

pub struct MappedRequester<'a, T>(Box<dyn Fn(T) -> RawReply + Send + Sync + 'a>);
impl<'a, T> MappedRequester<'a, T> {
  fn new<U: DynRequester + 'a>(req: U) -> Self
  where T: Into<U::Transfer> {
    MappedRequester(Box::new(move |t| req.raw_request(t.into())))
  }
}

impl<T> DynRequester for MappedRequester<'_, T> {
  type Transfer = T;
  fn raw_request(&self, data: Self::Transfer) -> RawReply { self.0(data) }
}

impl<T: MsgSet> DynRequester for ReqNot<T> {
  type Transfer = <T::Out as Channel>::Req;
  fn raw_request(&self, req: Self::Transfer) -> RawReply {
    let mut g = self.0.lock().unwrap();
    let id = g.id;
    g.id += 1;
    let mut buf = id.to_be_bytes().to_vec();
    req.encode(&mut buf);
    let (send, recv) = sync_channel(1);
    g.responses.insert(id, send);
    let mut send = clone_box(&*g.send);
    mem::drop(g);
    send(&buf, self.clone());
    RawReply(recv.recv().unwrap())
  }
}

pub trait Requester: DynRequester {
  #[must_use = "These types are subject to change with protocol versions. \
    If you don't want to use the return value, At a minimum, force the type."]
  fn request<R: Request + Into<Self::Transfer>>(&self, data: R) -> R::Response;
  fn map<'a, U: Into<Self::Transfer>>(self) -> MappedRequester<'a, U>
  where Self: Sized + 'a {
    MappedRequester::new(self)
  }
}
impl<This: DynRequester + ?Sized> Requester for This {
  fn request<R: Request + Into<Self::Transfer>>(&self, data: R) -> R::Response {
    R::Response::decode(&mut &self.raw_request(data.into())[..])
  }
}

impl<T: MsgSet> Clone for ReqNot<T> {
  fn clone(&self) -> Self { Self(self.0.clone()) }
}

#[cfg(test)]
mod test {
  use std::sync::{Arc, Mutex};

  use orchid_api_derive::Coding;
  use orchid_api_traits::{Channel, Request};

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
    let received = Arc::new(Mutex::new(None));
    let receiver = ReqNot::<TestMsgSet>::new(
      |_, _| panic!("Should not send anything"),
      clone!(received; move |notif, _| *received.lock().unwrap() = Some(notif)),
      |_, _| panic!("Not receiving a request"),
    );
    let sender = ReqNot::<TestMsgSet>::new(
      clone!(receiver; move |d, _| receiver.receive(d)),
      |_, _| panic!("Should not receive notif"),
      |_, _| panic!("Should not receive request"),
    );
    sender.notify(3);
    assert_eq!(*received.lock().unwrap(), Some(3));
    sender.notify(4);
    assert_eq!(*received.lock().unwrap(), Some(4));
  }

  #[test]
  fn request() {
    let receiver = Arc::new(Mutex::<Option<ReqNot<TestMsgSet>>>::new(None));
    let sender = Arc::new(ReqNot::<TestMsgSet>::new(
      {
        let receiver = receiver.clone();
        move |d, _| receiver.lock().unwrap().as_ref().unwrap().receive(d)
      },
      |_, _| panic!("Should not receive notif"),
      |_, _| panic!("Should not receive request"),
    ));
    *receiver.lock().unwrap() = Some(ReqNot::new(
      {
        let sender = sender.clone();
        move |d, _| sender.receive(d)
      },
      |_, _| panic!("Not receiving notifs"),
      |hand, req| {
        assert_eq!(req, TestReq(5));
        hand.respond(&6u8)
      },
    ));
    let response = sender.request(TestReq(5));
    assert_eq!(response, 6);
  }
}
