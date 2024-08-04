use super::coding::{Coding, Encode};

pub trait Request: Coding + Sized + Send + 'static {
  type Response: Coding + Send + 'static;
  fn respond(&self, rep: Self::Response) -> Vec<u8> { rep.enc_vec() }
}

pub trait Channel: 'static {
  type Req: Coding + Sized + Send + 'static;
  type Notif: Coding + Sized + Send + 'static;
}

pub trait MsgSet: Send + Sync + 'static {
  type In: Channel;
  type Out: Channel;
}
