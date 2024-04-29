use super::coding::{Coding, Encode};

pub trait Request: Coding + Sized + Send + 'static {
  type Response: Coding + Send + 'static;
  fn respond(&self, rep: Self::Response) -> Vec<u8> { rep.enc_vec() }
}

pub trait MsgSet {
  type InReq: Coding + Sized + Send + 'static;
  type InNot: Coding + Sized + Send + 'static;
  type OutReq: Coding + Sized + Send + 'static;
  type OutNot: Coding + Sized + Send + 'static;
}
