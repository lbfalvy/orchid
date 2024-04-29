use orchid_api_derive::Coding;
use orchid_api_traits::Request;

pub type FsId = u16;

#[derive(Clone, Debug, Coding)]
pub enum Loaded {
  Code(String),
  Collection(Vec<String>),
}

#[derive(Clone, Debug, Coding)]
pub struct FsRead(pub Vec<String>);
impl Request for FsRead {
  type Response = Result<Loaded, ()>;
}
