use std::rc::Rc;

use never::Never;
use orchid_base::number::Numeric;
use orchid_base::reqnot::Receipt;
use orchid_extension::atom::{AtomDynfo, AtomicFeatures};
use orchid_extension::entrypoint::ExtReq;
use orchid_extension::fs::DeclFs;
use orchid_extension::system::{System, SystemCard};
use orchid_extension::system_ctor::SystemCtor;
use orchid_extension::tree::{MemKind, comments, fun, module, root_mod};
use ordered_float::NotNan;

use crate::OrcString;
use crate::number::num_atom::{Float, HomoArray, Int, Num};
use crate::number::num_lexer::NumLexer;
use crate::string::str_atom::{IntStrAtom, StrAtom};
use crate::string::str_lexer::StringLexer;

#[derive(Default)]
pub struct StdSystem;
impl SystemCtor for StdSystem {
	type Deps = ();
	type Instance = Self;
	const NAME: &'static str = "orchid::std";
	const VERSION: f64 = 0.00_01;
	fn inst() -> Option<Self::Instance> { Some(StdSystem) }
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
	fn lexers() -> Vec<orchid_extension::lexer::LexerObj> { vec![&StringLexer, &NumLexer] }
	fn parsers() -> Vec<orchid_extension::parser::ParserObj> { vec![] }
	fn vfs() -> DeclFs { DeclFs::Mod(&[]) }
	fn env() -> Vec<(String, MemKind)> {
		vec![root_mod("std", [], [
			module(true, "string", [], [comments(
				["Concatenate two strings"],
				fun(true, "concat", |left: OrcString<'static>, right: OrcString<'static>| async move {
					StrAtom::new(Rc::new(left.get_string().await.to_string() + &right.get_string().await))
				}),
			)]),
			module(true, "number", [], [
				fun(true, "add", |a: Num, b: Num| async move {
					Num(match HomoArray::new([a.0, b.0]) {
						HomoArray::Int([a, b]) => Numeric::Int(a + b),
						HomoArray::Float([a, b]) => Numeric::Float(a + b),
					})
				}),
				fun(true, "neg", |a: Num| async move {
					Num(match a.0 {
						Numeric::Int(i) => Numeric::Int(-i),
						Numeric::Float(f) => Numeric::Float(-f),
					})
				}),
				fun(true, "mul", |a: Num, b: Num| async move {
					Num(match HomoArray::new([a.0, b.0]) {
						HomoArray::Int([a, b]) => Numeric::Int(a * b),
						HomoArray::Float([a, b]) => Numeric::Float(a * b),
					})
				}),
				fun(true, "idiv", |a: Int, b: Int| async move { Int(a.0 / b.0) }),
				fun(true, "imod", |a: Int, b: Int| async move { Int(a.0 % b.0) }),
				fun(true, "fdiv", |a: Float, b: Float| async move { Float(a.0 / b.0) }),
				fun(true, "fmod", |a: Float, b: Float| async move {
					Float(a.0 - NotNan::new((a.0 / b.0).trunc()).unwrap() * b.0)
				}),
			]),
		])]
	}
}
