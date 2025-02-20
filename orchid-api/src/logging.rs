use orchid_api_derive::{Coding, Hierarchy};

use crate::ExtHostNotif;

#[derive(Clone, Debug, Coding, PartialEq, Eq, Hash)]
pub enum LogStrategy {
	StdErr,
	File(String),
	Discard,
}

#[derive(Clone, Debug, Coding, Hierarchy)]
#[extends(ExtHostNotif)]
pub struct Log(pub String);
