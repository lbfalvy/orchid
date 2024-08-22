use std::marker::PhantomData;
use std::ops::{BitAnd, Deref};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::{mem, thread};

use dyn_clone::{clone_box, DynClone};
use hashbrown::HashMap;
use orchid_api_traits::{Channel, Coding, Decode, Encode, MsgSet, Request};
use trait_set::trait_set;

pub struct ReplyToken;

trait_set! {
  pub trait SendFn<T: MsgSet> = for<'a> FnMut(&'a [u8], ReqNot<T>) + DynClone + Send + 'static;
  pub trait ReqFn<T: MsgSet> =
    FnMut(RequestHandle<T>) -> ReplyToken + DynClone + Send + Sync + 'static;
  pub trait NotifFn<T: MsgSet> =
    for<'a> FnMut(<T::In as Channel>::Notif, ReqNot<T>) + DynClone + Send + Sync + 'static;
}

fn get_id(message: &[u8]) -> (u64, &[u8]) {
  (u64::from_be_bytes(message[..8].to_vec().try_into().unwrap()), &message[8..])
}

pub struct RequestHandle<T: MsgSet> {
  id: u64,
  message: <T::In as Channel>::Req,
  parent: ReqNot<T>,
  fulfilled: AtomicBool,
}
impl<MS: MsgSet + 'static> RequestHandle<MS> {
  pub fn reqnot(&self) -> ReqNot<MS> { self.parent.clone() }
  pub fn req(&self) -> &<MS::In as Channel>::Req { &self.message }
  fn respond(&self, response: &impl Encode) -> ReplyToken {
    assert!(!self.fulfilled.swap(true, Ordering::Relaxed), "Already responded to {}", self.id);
    let mut buf = (!self.id).to_be_bytes().to_vec();
    response.encode(&mut buf);
    let mut send = clone_box(&*self.reqnot().0.lock().unwrap().send);
    (send)(&buf, self.parent.clone());
    ReplyToken
  }
  pub fn handle<T: Request>(&self, _: &T, rep: &T::Response) -> ReplyToken { self.respond(rep) }
  pub fn will_handle_as<T: Request>(&self, _: &T) -> ReqTypToken<T> { ReqTypToken(PhantomData) }
  pub fn handle_as<T: Request>(&self, _token: ReqTypToken<T>, rep: &T::Response) -> ReplyToken {
    self.respond(rep)
  }
}
impl<MS: MsgSet> Drop for RequestHandle<MS> {
  fn drop(&mut self) {
    let done = self.fulfilled.load(Ordering::Relaxed);
    debug_assert!(done, "Request {} dropped without response", self.id)
  }
}

pub struct ReqTypToken<T>(PhantomData<T>);

pub fn respond_with<R: Request>(r: &R, f: impl FnOnce(&R) -> R::Response) -> Vec<u8> {
  r.respond(f(r))
}

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
  pub fn receive(&self, message: Vec<u8>) {
    let mut g = self.0.lock().unwrap();
    let (id, payload) = get_id(&message[..]);
    if id == 0 {
      let mut notif = clone_box(&*g.notif);
      mem::drop(g);
      notif(<T::In as Channel>::Notif::decode(&mut &payload[..]), self.clone())
    } else if 0 < id.bitand(1 << 63) {
      let sender = g.responses.remove(&!id).expect("Received response for invalid message");
      sender.send(message).unwrap();
    } else {
      let message = <T::In as Channel>::Req::decode(&mut &payload[..]);
      let mut req = clone_box(&*g.req);
      mem::drop(g);
      let handle = RequestHandle { id, message, fulfilled: false.into(), parent: self.clone() };
      thread::Builder::new().name(format!("request {id}")).spawn(move || req(handle)).unwrap();
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

impl<'a, T> DynRequester for MappedRequester<'a, T> {
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
impl<'a, This: DynRequester + ?Sized + 'a> Requester for This {
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
      |_| panic!("Not receiving a request"),
    );
    let sender = ReqNot::<TestMsgSet>::new(
      clone!(receiver; move |d, _| receiver.receive(d.to_vec())),
      |_, _| panic!("Should not receive notif"),
      |_| panic!("Should not receive request"),
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
        move |d, _| receiver.lock().unwrap().as_ref().unwrap().receive(d.to_vec())
      },
      |_, _| panic!("Should not receive notif"),
      |_| panic!("Should not receive request"),
    ));
    *receiver.lock().unwrap() = Some(ReqNot::new(
      {
        let sender = sender.clone();
        move |d, _| sender.receive(d.to_vec())
      },
      |_, _| panic!("Not receiving notifs"),
      |req| {
        assert_eq!(req.req(), &TestReq(5));
        req.respond(&6u8)
      },
    ));
    let response = sender.request(TestReq(5));
    assert_eq!(response, 6);
  }
}
