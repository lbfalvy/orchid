use orchid_api::PhKind;
use orchid_base::interner::Tok;
use orchid_base::tree::Ph;

use crate::macros::{MacTok, MacTree};

/// Returns the name, priority and at_least_one of the expression if it is
/// a vectorial placeholder
#[must_use]
pub fn vec_attrs(expr: &MacTree) -> Option<(Tok<String>, u8, bool)> {
	match (*expr.tok).clone() {
		MacTok::Ph(Ph { kind: PhKind::Vector { priority, at_least_one }, name }) =>
			Some((name, priority, at_least_one)),
		_ => None,
	}
}
