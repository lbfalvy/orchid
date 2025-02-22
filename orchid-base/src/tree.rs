use std::borrow::Borrow;
use std::fmt::{self, Debug, Display};
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

pub use api::PhKind;
use async_stream::stream;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use itertools::Itertools;
use never::Never;
use ordered_float::NotNan;
use trait_set::trait_set;

use crate::error::OrcErrv;
use crate::format::{FmtCtx, FmtUnit, Format, Variants};
use crate::interner::{Interner, Tok};
use crate::location::Pos;
use crate::name::Sym;
use crate::parse::Snippet;
use crate::{api, match_mapping, tl_cache};

trait_set! {
	pub trait RecurCB<'a, A: AtomRepr, X: ExtraTok> = Fn(TokTree<'a, A, X>) -> TokTree<'a, A, X>;
	pub trait ExtraTok = Format + Clone + fmt::Debug;
	pub trait RefDoExtra<X> = AsyncFnMut(&X, Range<u32>) -> api::TokenTree;
}

pub fn recur<'a, A: AtomRepr, X: ExtraTok>(
	tt: TokTree<'a, A, X>,
	f: &impl Fn(TokTree<'a, A, X>, &dyn RecurCB<'a, A, X>) -> TokTree<'a, A, X>,
) -> TokTree<'a, A, X> {
	f(tt, &|TokTree { range, tok }| {
		let tok = match tok {
			tok @ (Token::Atom(_) | Token::BR | Token::Bottom(_) | Token::Comment(_) | Token::NS) => tok,
			tok @ (Token::Name(_) | Token::Slot(_) | Token::X(_) | Token::Ph(_) | Token::Macro(_)) => tok,
			tok @ Token::Reference(_) => tok,
			Token::LambdaHead(arg) =>
				Token::LambdaHead(arg.into_iter().map(|tt| recur(tt, f)).collect_vec()),
			Token::S(p, b) => Token::S(p, b.into_iter().map(|tt| recur(tt, f)).collect_vec()),
		};
		TokTree { range, tok }
	})
}

pub trait AtomRepr: Clone + Format {
	type Ctx: ?Sized;
	fn from_api(api: &api::Atom, pos: Pos, ctx: &mut Self::Ctx) -> impl Future<Output = Self>;
	fn to_api(&self) -> impl Future<Output = orchid_api::Atom> + '_;
}
impl AtomRepr for Never {
	type Ctx = Never;
	async fn from_api(_: &api::Atom, _: Pos, ctx: &mut Self::Ctx) -> Self { match *ctx {} }
	async fn to_api(&self) -> orchid_api::Atom { match *self {} }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TokHandle<'a>(api::TreeTicket, PhantomData<&'a ()>);
impl TokHandle<'static> {
	pub fn new(tt: api::TreeTicket) -> Self { TokHandle(tt, PhantomData) }
}
impl TokHandle<'_> {
	pub fn ticket(self) -> api::TreeTicket { self.0 }
}
impl Display for TokHandle<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Handle({})", self.0.0) }
}

#[derive(Clone, Debug)]
pub struct TokTree<'a, A: AtomRepr, X: ExtraTok> {
	pub tok: Token<'a, A, X>,
	pub range: Range<u32>,
}
impl<'b, A: AtomRepr, X: ExtraTok> TokTree<'b, A, X> {
	pub async fn from_api(tt: &api::TokenTree, ctx: &mut A::Ctx, i: &Interner) -> Self {
		let tok = match_mapping!(&tt.token, api::Token => Token::<'b, A, X> {
			BR, NS,
			Atom(a => A::from_api(a, Pos::Range(tt.range.clone()), ctx).await),
			Bottom(e => OrcErrv::from_api(e, i).await),
			LambdaHead(arg => ttv_from_api(arg, ctx, i).await),
			Name(n => Tok::from_api(*n, i).await),
			S(*par, b => ttv_from_api(b, ctx, i).await),
			Comment(c.clone()),
			Slot(id => TokHandle::new(*id)),
			Ph(ph => Ph::from_api(ph, i).await),
			Macro(*prio),
			Reference(tok => Sym::from_api(*tok, i).await)
		});
		Self { range: tt.range.clone(), tok }
	}

	pub async fn to_api(&self, do_extra: &mut impl RefDoExtra<X>) -> api::TokenTree {
		let token = match_mapping!(&self.tok, Token => api::Token {
			Atom(a.to_api().await),
			BR,
			NS,
			Bottom(e.to_api()),
			Comment(c.clone()),
			LambdaHead(arg => ttv_to_api(arg, do_extra).boxed_local().await),
			Name(n.to_api()),
			Slot(tt.ticket()),
			S(*p, b => ttv_to_api(b, do_extra).boxed_local().await),
			Ph(ph.to_api()),
			Macro(*prio),
			Reference(sym.to_api()),
		} {
			Token::X(x) => return do_extra(x, self.range.clone()).await
		});
		api::TokenTree { range: self.range.clone(), token }
	}

	pub async fn into_api(
		self,
		do_extra: &mut impl FnMut(X, Range<u32>) -> api::TokenTree,
	) -> api::TokenTree {
		let token = match self.tok {
			Token::Atom(a) => api::Token::Atom(a.to_api().await),
			Token::Reference(sym) => api::Token::Reference(sym.to_api()),
			Token::BR => api::Token::BR,
			Token::NS => api::Token::NS,
			Token::Bottom(e) => api::Token::Bottom(e.to_api()),
			Token::Comment(c) => api::Token::Comment(c.clone()),
			Token::LambdaHead(arg) => api::Token::LambdaHead(ttv_into_api(arg, do_extra).await),
			Token::Name(n) => api::Token::Name(n.to_api()),
			Token::Slot(tt) => api::Token::Slot(tt.ticket()),
			Token::S(p, b) => api::Token::S(p, ttv_into_api(b, do_extra).await),
			Token::Ph(Ph { kind, name }) =>
				api::Token::Ph(api::Placeholder { name: name.to_api(), kind }),
			Token::X(x) => return do_extra(x, self.range.clone()),
			Token::Macro(prio) => api::Token::Macro(prio),
		};
		api::TokenTree { range: self.range.clone(), token }
	}

	pub fn is_kw(&self, tk: Tok<String>) -> bool { self.tok.is_kw(tk) }
	pub fn as_name(&self) -> Option<Tok<String>> {
		if let Token::Name(n) = &self.tok { Some(n.clone()) } else { None }
	}
	pub fn as_s<'a>(&'a self, par: Paren, i: &'a Interner) -> Option<Snippet<'a, 'b, A, X>> {
		self.tok.as_s(par).map(|slc| Snippet::new(self, slc, i))
	}
	pub fn lambda(arg: Vec<Self>, mut body: Vec<Self>) -> Self {
		let arg_range = ttv_range(&arg);
		let s_range = arg_range.start..body.last().expect("Lambda with empty body!").range.end;
		body.insert(0, Token::LambdaHead(arg).at(arg_range));
		Token::S(Paren::Round, body).at(s_range)
	}
}
impl<A: AtomRepr, X: ExtraTok> Format for TokTree<'_, A, X> {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		self.tok.print(c).await
	}
}

pub async fn ttv_from_api<A: AtomRepr, X: ExtraTok>(
	tokv: impl IntoIterator<Item: Borrow<api::TokenTree>>,
	ctx: &mut A::Ctx,
	i: &Interner,
) -> Vec<TokTree<'static, A, X>> {
	stream! {
		for tok in tokv {
			yield TokTree::<A, X>::from_api(tok.borrow(), ctx, i).boxed_local().await
		}
	}
	.collect()
	.await
}

pub async fn ttv_to_api<'a, A: AtomRepr, X: ExtraTok>(
	tokv: impl IntoIterator<Item: Borrow<TokTree<'a, A, X>>>,
	do_extra: &mut impl RefDoExtra<X>,
) -> Vec<api::TokenTree> {
	let mut output = Vec::new();
	for tok in tokv {
		output.push(Borrow::<TokTree<A, X>>::borrow(&tok).to_api(do_extra).await)
	}
	output
}

pub async fn ttv_into_api<'a, A: AtomRepr, X: ExtraTok>(
	tokv: impl IntoIterator<Item = TokTree<'a, A, X>>,
	do_extra: &mut impl FnMut(X, Range<u32>) -> api::TokenTree,
) -> Vec<api::TokenTree> {
	stream! {
		for tok in tokv {
			yield tok.into_api(do_extra).await
		}
	}
	.collect()
	.await
}

pub fn wrap_tokv<'a, A: AtomRepr, X: ExtraTok>(
	items: impl IntoIterator<Item = TokTree<'a, A, X>>,
) -> TokTree<'a, A, X> {
	let items_v = items.into_iter().collect_vec();
	match items_v.len() {
		0 => panic!("A tokv with no elements is illegal"),
		1 => items_v.into_iter().next().unwrap(),
		_ => {
			let range = items_v.first().unwrap().range.start..items_v.last().unwrap().range.end;
			Token::S(api::Paren::Round, items_v).at(range)
		},
	}
}

pub use api::Paren;

/// Lexer output variant
#[derive(Clone, Debug)]
pub enum Token<'a, A: AtomRepr, X: ExtraTok> {
	/// Information about the code addressed to the human reader or dev tooling
	/// It has no effect on the behaviour of the program unless it's explicitly
	/// read via reflection
	Comment(Arc<String>),
	/// The part of a lambda between `\` and `.` enclosing the argument. The body
	/// stretches to the end of the enclosing parens or the end of the const line
	LambdaHead(Vec<TokTree<'a, A, X>>),
	/// A binding, operator, or a segment of a namespaced::name
	Name(Tok<String>),
	/// The namespace separator ::
	NS,
	/// A line break
	BR,
	/// `()`, `[]`, or `{}`
	S(Paren, Vec<TokTree<'a, A, X>>),
	/// A fully formed reference to external code emitted by a lexer plugin
	Reference(Sym),
	/// A value emitted by a lexer plugin
	Atom(A),
	/// A grammar error emitted by a lexer plugin if it was possible to continue
	/// reading. Parsers should treat it as an atom unless it prevents parsing,
	/// in which case both this and a relevant error should be returned.
	Bottom(OrcErrv),
	/// An instruction from a plugin for the lexer to embed a subexpression
	/// without retransmitting it. It should not appear anywhere outside lexer
	/// plugin responses.
	Slot(TokHandle<'a>),
	/// Additional domain-specific token types
	X(X),
	/// A placeholder for metaprogramming, either $name, ..$name, ..$name:N,
	/// ...$name, or ...$name:N
	Ph(Ph),
	/// `macro` or `macro(`X`)` where X is any valid floating point number
	/// expression. `macro` is not a valid name in Orchid for this reason.
	Macro(Option<NotNan<f64>>),
}
impl<'a, A: AtomRepr, X: ExtraTok> Token<'a, A, X> {
	pub fn at(self, range: Range<u32>) -> TokTree<'a, A, X> { TokTree { range, tok: self } }
	pub fn is_kw(&self, tk: Tok<String>) -> bool { matches!(self, Token::Name(n) if *n == tk) }
	pub fn as_s(&self, par: Paren) -> Option<&[TokTree<'a, A, X>]> {
		match self {
			Self::S(p, b) if *p == par => Some(b),
			_ => None,
		}
	}
}
impl<A: AtomRepr, X: ExtraTok> Format for Token<'_, A, X> {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		match self {
			Self::Atom(a) => a.print(c).await,
			Self::BR => "\n".to_string().into(),
			Self::Bottom(err) if err.len() == 1 => format!("Bottom({}) ", err.one().unwrap()).into(),
			Self::Bottom(err) => format!("Botttom(\n{}) ", indent(&err.to_string())).into(),
			Self::Comment(c) => format!("--[{c}]--").into(),
			Self::LambdaHead(arg) =>
				FmtUnit::new(tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("\\{0b}."))), [
					ttv_fmt(arg, c).await,
				]),
			Self::NS => "::".to_string().into(),
			Self::Name(n) => format!("{n}").into(),
			Self::Reference(sym) => format!("{sym}").into(),
			Self::Slot(th) => format!("{th}").into(),
			Self::Ph(ph) => format!("{ph}").into(),
			Self::S(p, b) => FmtUnit::new(
				match *p {
					Paren::Round => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("({0b})"))),
					Paren::Curly => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{{{0b}}}"))),
					Paren::Square => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("[{0b}]"))),
				},
				[ttv_fmt(b, c).await],
			),
			Self::X(x) => x.print(c).await,
			Self::Macro(None) => "macro".to_string().into(),
			Self::Macro(Some(prio)) => format!("macro({prio})").into(),
		}
	}
}

pub fn ttv_range(ttv: &[TokTree<'_, impl AtomRepr, impl ExtraTok>]) -> Range<u32> {
	assert!(!ttv.is_empty(), "Empty slice has no range");
	ttv.first().unwrap().range.start..ttv.last().unwrap().range.end
}

pub async fn ttv_fmt<'a: 'b, 'b>(
	ttv: impl IntoIterator<Item = &'b TokTree<'a, impl AtomRepr + 'b, impl ExtraTok + 'b>>,
	c: &(impl FmtCtx + ?Sized),
) -> FmtUnit {
	FmtUnit::sequence(" ", None, join_all(ttv.into_iter().map(|t| t.print(c))).await)
}

pub fn indent(s: &str) -> String { s.replace("\n", "\n  ") }

#[derive(Clone, Debug)]
pub struct Ph {
	pub name: Tok<String>,
	pub kind: PhKind,
}
impl Ph {
	pub async fn from_api(api: &api::Placeholder, i: &Interner) -> Self {
		Self { name: Tok::from_api(api.name, i).await, kind: api.kind }
	}
	pub fn to_api(&self) -> api::Placeholder {
		api::Placeholder { name: self.name.to_api(), kind: self.kind }
	}
}
impl Display for Ph {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let n = &self.name;
		match self.kind {
			PhKind::Scalar => write!(f, "${n}"),
			PhKind::Vector { priority: 0, at_least_one: true } => write!(f, "...${}", self.name),
			PhKind::Vector { priority: p, at_least_one: true } => write!(f, "...${}:{}", self.name, p),
			PhKind::Vector { priority: 0, at_least_one: false } => write!(f, "..${}", self.name),
			PhKind::Vector { priority: p, at_least_one: false } => write!(f, "..${}:{}", self.name, p),
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_covariance() {
		fn _f<'a>(x: Token<'static, Never, Never>) -> Token<'a, Never, Never> { x }
	}

	#[test]
	fn fail_covariance() {
		// this fails to compile
		// fn _f<'a, 'b>(x: &'a mut &'static ()) -> &'a mut &'b () { x }
		// this passes because it's covariant
		fn _f<'a, 'b>(x: &'a fn() -> &'static ()) -> &'a fn() -> &'b () { x }
	}
}
