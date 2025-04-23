use std::borrow::Borrow;
use std::fmt::{self, Debug, Display};
use std::future::Future;
use std::marker::PhantomData;
use std::ops::Range;
use std::rc::Rc;

use async_stream::stream;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use itertools::Itertools;
use never::Never;
use orchid_api_traits::Coding;
use trait_set::trait_set;

use crate::error::OrcErrv;
use crate::format::{FmtCtx, FmtUnit, Format, Variants};
use crate::interner::{Interner, Tok};
use crate::location::Pos;
use crate::parse::Snippet;
use crate::{api, match_mapping, tl_cache};

pub trait TokenVariant<ApiEquiv: Clone + Debug + Coding>: Format + Clone + fmt::Debug {
	type FromApiCtx<'a>;
	type ToApiCtx<'a>;
	fn from_api(
		api: &ApiEquiv,
		ctx: &mut Self::FromApiCtx<'_>,
		pos: Pos,
		i: &Interner,
	) -> impl Future<Output = Self>;
	fn into_api(self, ctx: &mut Self::ToApiCtx<'_>) -> impl Future<Output = ApiEquiv>;
}
impl<T: Clone + Debug + Coding> TokenVariant<T> for Never {
	type FromApiCtx<'a> = ();
	type ToApiCtx<'a> = ();
	async fn from_api(_: &T, _: &mut Self::FromApiCtx<'_>, _: Pos, _: &Interner) -> Self {
		panic!("Cannot deserialize Never")
	}
	async fn into_api(self, _: &mut Self::ToApiCtx<'_>) -> T { match self {} }
}

trait_set! {
	// TokenHandle
	pub trait ExprRepr = TokenVariant<api::ExprTicket>;
	// TokenExpr
	pub trait ExtraTok = TokenVariant<api::Expression>;
}

trait_set! {
	pub trait RecurCB<H: ExprRepr, X: ExtraTok> = Fn(TokTree<H, X>) -> TokTree<H, X>;
}

pub fn recur<H: ExprRepr, X: ExtraTok>(
	tt: TokTree<H, X>,
	f: &impl Fn(TokTree<H, X>, &dyn RecurCB<H, X>) -> TokTree<H, X>,
) -> TokTree<H, X> {
	f(tt, &|TokTree { range, tok }| {
		let tok = match tok {
			tok @ (Token::BR | Token::Bottom(_) | Token::Comment(_) | Token::Name(_)) => tok,
			tok @ (Token::Handle(_) | Token::NewExpr(_)) => tok,
			Token::NS(n, b) => Token::NS(n, Box::new(recur(*b, f))),
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
pub struct TokTree<H: ExprRepr, X: ExtraTok> {
	pub tok: Token<H, X>,
	pub range: Range<u32>,
}
impl<H: ExprRepr, X: ExtraTok> TokTree<H, X> {
	pub async fn from_api(
		tt: &api::TokenTree,
		hctx: &mut H::FromApiCtx<'_>,
		xctx: &mut X::FromApiCtx<'_>,
		i: &Interner,
	) -> Self {
		let tok = match_mapping!(&tt.token, api::Token => Token::<H, X> {
			BR,
			NS(n => Tok::from_api(*n, i).await,
				b => Box::new(Self::from_api(b, hctx, xctx, i).boxed_local().await)),
			Bottom(e => OrcErrv::from_api(e, i).await),
			LambdaHead(arg => ttv_from_api(arg, hctx, xctx, i).await),
			Name(n => Tok::from_api(*n, i).await),
			S(*par, b => ttv_from_api(b, hctx, xctx, i).await),
			Comment(c.clone()),
			NewExpr(expr => X::from_api(expr, xctx, Pos::Range(tt.range.clone()), i).await),
			Handle(tk => H::from_api(tk, hctx, Pos::Range(tt.range.clone()), i).await)
		});
		Self { range: tt.range.clone(), tok }
	}

	pub async fn into_api(
		self,
		hctx: &mut H::ToApiCtx<'_>,
		xctx: &mut X::ToApiCtx<'_>,
	) -> api::TokenTree {
		let token = match_mapping!(self.tok, Token => api::Token {
			BR,
			NS(n.to_api(), b => Box::new(b.into_api(hctx, xctx).boxed_local().await)),
			Bottom(e.to_api()),
			Comment(c.clone()),
			LambdaHead(arg => ttv_into_api(arg, hctx, xctx).await),
			Name(nn.to_api()),
			S(p, b => ttv_into_api(b, hctx, xctx).await),
			Handle(hand.into_api(hctx).await),
			NewExpr(expr.into_api(xctx).await),
		});
		api::TokenTree { range: self.range.clone(), token }
	}

	pub fn is_kw(&self, tk: Tok<String>) -> bool { self.tok.is_kw(tk) }
	pub fn as_name(&self) -> Option<Tok<String>> {
		if let Token::Name(n) = &self.tok { Some(n.clone()) } else { None }
	}
	pub fn as_s(&self, par: Paren) -> Option<Snippet<'_, H, X>> {
		self.tok.as_s(par).map(|slc| Snippet::new(self, slc))
	}
	pub fn as_lambda(&self) -> Option<Snippet<'_, H, X>> {
		match &self.tok {
			Token::LambdaHead(arg) => Some(Snippet::new(self, arg)),
			_ => None,
		}
	}
	pub fn is_fluff(&self) -> bool { matches!(self.tok, Token::Comment(_) | Token::BR) }
	pub fn lambda(arg: Vec<Self>, mut body: Vec<Self>) -> Self {
		let arg_range = ttv_range(&arg);
		let s_range = arg_range.start..body.last().expect("Lambda with empty body!").range.end;
		body.insert(0, Token::LambdaHead(arg).at(arg_range));
		Token::S(Paren::Round, body).at(s_range)
	}
}
impl<H: ExprRepr, X: ExtraTok> Format for TokTree<H, X> {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		self.tok.print(c).await
	}
}

pub async fn ttv_from_api<H: ExprRepr, X: ExtraTok>(
	tokv: impl IntoIterator<Item: Borrow<api::TokenTree>>,
	hctx: &mut H::FromApiCtx<'_>,
	xctx: &mut X::FromApiCtx<'_>,
	i: &Interner,
) -> Vec<TokTree<H, X>> {
	stream! {
		for tok in tokv {
			yield TokTree::<H, X>::from_api(tok.borrow(), hctx, xctx, i).boxed_local().await
		}
	}
	.collect()
	.await
}

pub async fn ttv_into_api<H: ExprRepr, X: ExtraTok>(
	tokv: impl IntoIterator<Item = TokTree<H, X>>,
	hctx: &mut H::ToApiCtx<'_>,
	xctx: &mut X::ToApiCtx<'_>,
) -> Vec<api::TokenTree> {
	stream! {
		for tok in tokv {
			yield tok.into_api(hctx, xctx).await
		}
	}
	.collect()
	.await
}

pub fn wrap_tokv<H: ExprRepr, X: ExtraTok>(
	items: impl IntoIterator<Item = TokTree<H, X>>,
) -> TokTree<H, X> {
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
pub enum Token<H: ExprRepr, X: ExtraTok> {
	/// Information about the code addressed to the human reader or dev tooling
	/// It has no effect on the behaviour of the program unless it's explicitly
	/// read via reflection
	Comment(Rc<String>),
	/// The part of a lambda between `\` and `.` enclosing the argument. The body
	/// stretches to the end of the enclosing parens or the end of the const line
	LambdaHead(Vec<TokTree<H, X>>),
	/// A binding, operator, or a segment of a namespaced::name
	Name(Tok<String>),
	/// A namespace prefix, like `my_ns::` followed by a token
	NS(Tok<String>, Box<TokTree<H, X>>),
	/// A line break
	BR,
	/// `()`, `[]`, or `{}`
	S(Paren, Vec<TokTree<H, X>>),
	/// A newly instantiated expression
	NewExpr(X),
	/// An existing expr from a nested lexer
	Handle(H),
	/// A grammar error emitted by a lexer plugin if it was possible to continue
	/// reading. Parsers should treat it as an atom unless it prevents parsing,
	/// in which case both this and a relevant error should be returned.
	Bottom(OrcErrv),
}
impl<H: ExprRepr, X: ExtraTok> Token<H, X> {
	pub fn at(self, range: Range<u32>) -> TokTree<H, X> { TokTree { range, tok: self } }
	pub fn is_kw(&self, tk: Tok<String>) -> bool { matches!(self, Token::Name(n) if *n == tk) }
	pub fn as_s(&self, par: Paren) -> Option<&[TokTree<H, X>]> {
		match self {
			Self::S(p, b) if *p == par => Some(b),
			_ => None,
		}
	}
}
impl<H: ExprRepr, X: ExtraTok> Format for Token<H, X> {
	async fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit {
		match self {
			Self::BR => "\n".to_string().into(),
			Self::Bottom(err) if err.len() == 1 => format!("Bottom({}) ", err.one().unwrap()).into(),
			Self::Bottom(err) => format!("Botttom(\n{}) ", indent(&err.to_string())).into(),
			Self::Comment(c) => format!("--[{c}]--").into(),
			Self::LambdaHead(arg) =>
				tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("\\{0b}.")))
					.units([ttv_fmt(arg, c).await]),
			Self::NS(n, b) => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{0}::{1l}")))
				.units([n.to_string().into(), b.print(c).boxed_local().await]),
			Self::Name(n) => format!("{n}").into(),
			Self::S(p, b) => FmtUnit::new(
				match *p {
					Paren::Round => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("({0b})"))),
					Paren::Curly => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("{{{0b}}}"))),
					Paren::Square => tl_cache!(Rc<Variants>: Rc::new(Variants::default().bounded("[{0b}]"))),
				},
				[ttv_fmt(b, c).await],
			),
			Self::Handle(h) => h.print(c).await,
			Self::NewExpr(ex) => ex.print(c).await,
		}
	}
}

pub fn ttv_range<'a>(ttv: &[TokTree<impl ExprRepr + 'a, impl ExtraTok + 'a>]) -> Range<u32> {
	assert!(!ttv.is_empty(), "Empty slice has no range");
	ttv.first().unwrap().range.start..ttv.last().unwrap().range.end
}

pub async fn ttv_fmt<'a: 'b, 'b>(
	ttv: impl IntoIterator<Item = &'b TokTree<impl ExprRepr + 'a, impl ExtraTok + 'a>>,
	c: &(impl FmtCtx + ?Sized),
) -> FmtUnit {
	FmtUnit::sequence(" ", None, join_all(ttv.into_iter().map(|t| t.print(c))).await)
}

pub fn indent(s: &str) -> String { s.replace("\n", "\n  ") }
