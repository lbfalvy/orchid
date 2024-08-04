use orchid_api_derive::{Coding, Hierarchy};

use crate::proto::ExtHostNotif;

#[derive(Clone, Debug, Coding)]
pub enum LogStrategy {
  StdErr,
  File(String)
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostNotif)]
pub struct Log(pub String);