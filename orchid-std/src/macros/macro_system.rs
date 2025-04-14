use never::Never;
use orchid_base::reqnot::Receipt;
use orchid_extension::atom::AtomDynfo;
use orchid_extension::entrypoint::ExtReq;
use orchid_extension::fs::DeclFs;
use orchid_extension::lexer::LexerObj;
use orchid_extension::parser::ParserObj;
use orchid_extension::system::{System, SystemCard};
use orchid_extension::system_ctor::SystemCtor;
use orchid_extension::tree::GenItem;

#[derive(Default)]
pub struct MacroSystem;
impl SystemCtor for MacroSystem {
	type Deps = ();
	type Instance = Self;
	const NAME: &'static str = "macros";
	const VERSION: f64 = 0.00_01;
	fn inst() -> Option<Self::Instance> { Some(Self) }
}
impl SystemCard for MacroSystem {
	type Ctor = Self;
	type Req = Never;
	fn atoms() -> impl IntoIterator<Item = Option<Box<dyn AtomDynfo>>> { [] }
}
impl System for MacroSystem {
	async fn request(_: ExtReq<'_>, req: Self::Req) -> Receipt<'_> { match req {} }
	fn vfs() -> orchid_extension::fs::DeclFs { DeclFs::Mod(&[]) }
	fn lexers() -> Vec<LexerObj> { vec![] }
	fn parsers() -> Vec<ParserObj> { vec![] }
	fn env() -> Vec<GenItem> { vec![] }
}
