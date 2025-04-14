use never::Never;
use orchid_base::reqnot::Receipt;
use orchid_extension::atom::{AtomDynfo, AtomicFeatures};
use orchid_extension::entrypoint::ExtReq;
use orchid_extension::fs::DeclFs;
use orchid_extension::lexer::LexerObj;
use orchid_extension::parser::ParserObj;
use orchid_extension::system::{System, SystemCard};
use orchid_extension::system_ctor::SystemCtor;
use orchid_extension::tree::{GenItem, merge_trivial};

use super::number::num_lib::gen_num_lib;
use super::string::str_atom::{IntStrAtom, StrAtom};
use super::string::str_lib::gen_str_lib;
use crate::std::number::num_lexer::NumLexer;
use crate::std::string::str_lexer::StringLexer;
use crate::{Float, Int};

#[derive(Default)]
pub struct StdSystem;
impl SystemCtor for StdSystem {
	type Deps = ();
	type Instance = Self;
	const NAME: &'static str = "orchid::std";
	const VERSION: f64 = 0.00_01;
	fn inst() -> Option<Self::Instance> { Some(Self) }
}
impl SystemCard for StdSystem {
	type Ctor = Self;
	type Req = Never;
	fn atoms() -> impl IntoIterator<Item = Option<Box<dyn AtomDynfo>>> {
		[Some(Int::dynfo()), Some(Float::dynfo()), Some(StrAtom::dynfo()), Some(IntStrAtom::dynfo())]
	}
}
impl System for StdSystem {
	async fn request(_: ExtReq<'_>, req: Self::Req) -> Receipt<'_> { match req {} }
	fn lexers() -> Vec<LexerObj> { vec![&StringLexer, &NumLexer] }
	fn parsers() -> Vec<ParserObj> { vec![] }
	fn vfs() -> DeclFs { DeclFs::Mod(&[]) }
	fn env() -> Vec<GenItem> { merge_trivial([gen_num_lib(), gen_str_lib()]) }
}
