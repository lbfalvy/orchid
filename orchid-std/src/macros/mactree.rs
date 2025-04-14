use std::borrow::Cow;
use std::rc::Rc;

use futures::future::join_all;
use orchid_api::Paren;
use orchid_base::error::OrcErrv;
use orchid_base::format::{FmtCtx, FmtUnit, Format, Variants};
use orchid_base::location::Pos;
use orchid_base::name::Sym;
use orchid_base::tl_cache;
use orchid_base::tree::Ph;
use orchid_extension::atom::{Atomic, MethodSetBuilder};
use orchid_extension::atom_owned::{OwnedAtom, OwnedVariant};
use orchid_extension::expr::Expr;

#[derive(Debug, Clone)]
pub struct MacTree {
	pub pos: Pos,
	pub tok: Rc<MacTok>,
}
impl MacTree {}
impl Atomic for MacTree {
	type Data = ();
	type Variant = OwnedVariant;
}
impl OwnedAtom for MacTree {
	type Refs = ();

	async fn val(&self) -> Cow<'_, Self::Data> { Cow::Owned(()) }
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		self.tok.print(c).await
	}
}

#[derive(Debug, Clone)]
pub enum MacTok {
	S(Paren, Vec<MacTree>),
	Name(Sym),
	/// Only permitted in arguments to `instantiate_tpl`
	Slot,
	Value(Expr),
	Lambda(Vec<MacTree>, Vec<MacTree>),
	Ph(Ph),
}
impl Format for MacTok {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		match self {
			Self::Value(v) => v.print(c).await,
			Self::Lambda(arg, b) => FmtUnit::new(
				tl_cache!(Rc<Variants>: Rc::new(Variants::default()
					.unbounded("\\{0b}.{1l}")
					.bounded("(\\{0b}.{1b})"))),
				[mtreev_fmt(arg, c).await, mtreev_fmt(b, c).await],
			),
			Self::Name(n) => format!("{n}").into(),
			Self::Ph(ph) => format!("{ph}").into(),
			Self::S(p, body) => FmtUnit::new(
				match *p {
					Paren::Round => Rc::new(Variants::default().bounded("({0b})")),
					Paren::Curly => Rc::new(Variants::default().bounded("{{0b}}")),
					Paren::Square => Rc::new(Variants::default().bounded("[{0b}]")),
				},
				[mtreev_fmt(body, c).await],
			),
			Self::Slot => format!("SLOT").into(),
		}
	}
}

pub async fn mtreev_fmt<'b>(
	v: impl IntoIterator<Item = &'b MacTree>,
	c: &(impl FmtCtx + ?Sized),
) -> FmtUnit {
	FmtUnit::sequence(" ", None, join_all(v.into_iter().map(|t| t.print(c))).await)
}
