use std::rc::Rc;

use orchid_extension::tree::{GenItem, comments, fun, prefix};

use super::str_atom::StrAtom;
use crate::OrcString;

pub fn gen_str_lib() -> Vec<GenItem> {
	prefix("std::string", [comments(
		["Concatenate two strings"],
		fun(true, "concat", |left: OrcString<'static>, right: OrcString<'static>| async move {
			StrAtom::new(Rc::new(left.get_string().await.to_string() + &right.get_string().await))
		}),
	)])
}
