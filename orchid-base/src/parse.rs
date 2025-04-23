use std::fmt::{self, Display};
use std::iter;
use std::ops::{Deref, Range};

use futures::FutureExt;
use futures::future::join_all;
use itertools::Itertools;

use crate::api;
use crate::error::{OrcRes, Reporter, mk_err, mk_errv};
use crate::format::fmt;
use crate::interner::{Interner, Tok};
use crate::location::Pos;
use crate::name::VPath;
use crate::tree::{ExprRepr, ExtraTok, Paren, TokTree, Token};

pub trait ParseCtx {
	fn i(&self) -> &Interner;
	fn reporter(&self) -> &Reporter;
}
pub struct ParseCtxImpl<'a> {
	pub i: &'a Interner,
	pub r: &'a Reporter,
}
impl ParseCtx for ParseCtxImpl<'_> {
	fn i(&self) -> &Interner { self.i }
	fn reporter(&self) -> &Reporter { self.r }
}

pub fn name_start(c: char) -> bool { c.is_alphabetic() || c == '_' }
pub fn name_char(c: char) -> bool { name_start(c) || c.is_numeric() }
pub fn op_char(c: char) -> bool { !name_char(c) && !c.is_whitespace() && !"()[]{}\\".contains(c) }
pub fn unrep_space(c: char) -> bool { c.is_whitespace() && !"\r\n".contains(c) }

/// A cheaply copiable subsection of a document that holds onto context data and
/// one token for error reporting on empty subsections.
#[derive(Debug)]
pub struct Snippet<'a, A: ExprRepr, X: ExtraTok> {
	prev: &'a TokTree<A, X>,
	cur: &'a [TokTree<A, X>],
}
impl<'a, A, X> Snippet<'a, A, X>
where
	A: ExprRepr,
	X: ExtraTok,
{
	pub fn new(prev: &'a TokTree<A, X>, cur: &'a [TokTree<A, X>]) -> Self { Self { prev, cur } }
	pub fn split_at(self, pos: u32) -> (Self, Self) {
		let Self { prev, cur } = self;
		let fst = Self { prev, cur: &cur[..pos as usize] };
		let new_prev = if pos == 0 { self.prev } else { &self.cur[pos as usize - 1] };
		let snd = Self { prev: new_prev, cur: &self.cur[pos as usize..] };
		(fst, snd)
	}
	pub fn find_idx(self, mut f: impl FnMut(&Token<A, X>) -> bool) -> Option<u32> {
		self.cur.iter().position(|t| f(&t.tok)).map(|t| t as u32)
	}
	pub fn get(self, idx: u32) -> Option<&'a TokTree<A, X>> { self.cur.get(idx as usize) }
	pub fn len(self) -> u32 { self.cur.len() as u32 }
	pub fn prev(self) -> &'a TokTree<A, X> { self.prev }
	pub fn pos(self) -> Range<u32> {
		(self.cur.first().map(|f| f.range.start..self.cur.last().unwrap().range.end))
			.unwrap_or(self.prev.range.clone())
	}
	pub fn pop_front(self) -> Option<(&'a TokTree<A, X>, Self)> {
		self.cur.first().map(|r| (r, self.split_at(1).1))
	}
	pub fn pop_back(self) -> Option<(Self, &'a TokTree<A, X>)> {
		self.cur.last().map(|r| (self.split_at(self.len() - 1).0, r))
	}
	pub fn split_once(self, f: impl FnMut(&Token<A, X>) -> bool) -> Option<(Self, Self)> {
		let idx = self.find_idx(f)?;
		Some((self.split_at(idx).0, self.split_at(idx + 1).1))
	}
	pub fn split(mut self, mut f: impl FnMut(&Token<A, X>) -> bool) -> impl Iterator<Item = Self> {
		iter::from_fn(move || {
			if self.is_empty() {
				return None;
			}
			let (ret, next) = self.split_once(&mut f).unwrap_or(self.split_at(self.len()));
			self = next;
			Some(ret)
		})
	}
	pub fn is_empty(self) -> bool { self.len() == 0 }
	pub fn skip_fluff(self) -> Self {
		let non_fluff_start = self.find_idx(|t| !matches!(t, Token::BR | Token::Comment(_)));
		self.split_at(non_fluff_start.unwrap_or(self.len())).1
	}
}
impl<A: ExprRepr, X: ExtraTok> Copy for Snippet<'_, A, X> {}
impl<A: ExprRepr, X: ExtraTok> Clone for Snippet<'_, A, X> {
	fn clone(&self) -> Self { *self }
}
impl<A: ExprRepr, X: ExtraTok> Deref for Snippet<'_, A, X> {
	type Target = [TokTree<A, X>];
	fn deref(&self) -> &Self::Target { self.cur }
}

/// Remove tokens that aren't meaningful in expression context, such as comments
/// or line breaks
pub fn strip_fluff<A: ExprRepr, X: ExtraTok>(tt: &TokTree<A, X>) -> Option<TokTree<A, X>> {
	let tok = match &tt.tok {
		Token::BR => return None,
		Token::Comment(_) => return None,
		Token::LambdaHead(arg) => Token::LambdaHead(arg.iter().filter_map(strip_fluff).collect()),
		Token::S(p, b) => Token::S(*p, b.iter().filter_map(strip_fluff).collect()),
		t => t.clone(),
	};
	Some(TokTree { tok, range: tt.range.clone() })
}

#[derive(Clone, Debug)]
pub struct Comment {
	pub text: Tok<String>,
	pub range: Range<u32>,
}
impl Comment {
	pub async fn from_api(c: &api::Comment, i: &Interner) -> Self {
		Self { text: i.ex(c.text).await, range: c.range.clone() }
	}
	pub async fn from_tk(tk: &TokTree<impl ExprRepr, impl ExtraTok>, i: &Interner) -> Option<Self> {
		match &tk.tok {
			Token::Comment(text) => Some(Self { text: i.i(&**text).await, range: tk.range.clone() }),
			_ => None,
		}
	}
	pub fn to_tk<R: ExprRepr, X: ExtraTok>(&self) -> TokTree<R, X> {
		TokTree { tok: Token::Comment(self.text.rc().clone()), range: self.range.clone() }
	}
	pub fn to_api(&self) -> api::Comment {
		api::Comment { range: self.range.clone(), text: self.text.to_api() }
	}
}

impl fmt::Display for Comment {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "--[{}]--", self.text) }
}

pub async fn line_items<'a, A: ExprRepr, X: ExtraTok>(
	ctx: &impl ParseCtx,
	snip: Snippet<'a, A, X>,
) -> Vec<Parsed<'a, Vec<Comment>, A, X>> {
	let mut items = Vec::new();
	let mut comments = Vec::new();
	for mut line in snip.split(|t| matches!(t, Token::BR)) {
		match &line.cur {
			[TokTree { tok: Token::S(Paren::Round, tokens), .. }] => line.cur = tokens,
			[] => continue,
			_ => (),
		}
		match line.find_idx(|t| !matches!(t, Token::Comment(_))) {
			None => comments.extend(line.cur),
			Some(i) => {
				let (cmts, tail) = line.split_at(i);
				let comments = join_all(comments.drain(..).chain(cmts.cur).map(|t| async {
					Comment::from_tk(t, ctx.i()).await.expect("All are comments checked above")
				}))
				.await;
				items.push(Parsed { output: comments, tail });
			},
		}
	}
	items
}

pub async fn try_pop_no_fluff<'a, A: ExprRepr, X: ExtraTok>(
	ctx: &impl ParseCtx,
	snip: Snippet<'a, A, X>,
) -> ParseRes<'a, &'a TokTree<A, X>, A, X> {
	match snip.skip_fluff().pop_front() {
		Some((output, tail)) => Ok(Parsed { output, tail }),
		None => Err(mk_errv(ctx.i().i("Unexpected end").await, "Pattern ends abruptly", [Pos::Range(
			snip.pos(),
		)
		.into()])),
	}
}

pub async fn expect_end(
	ctx: &impl ParseCtx,
	snip: Snippet<'_, impl ExprRepr, impl ExtraTok>,
) -> OrcRes<()> {
	match snip.skip_fluff().get(0) {
		Some(surplus) => Err(mk_errv(
			ctx.i().i("Extra code after end of line").await,
			"Code found after the end of the line",
			[Pos::Range(surplus.range.clone()).into()],
		)),
		None => Ok(()),
	}
}

pub async fn expect_tok<'a, A: ExprRepr, X: ExtraTok>(
	ctx: &impl ParseCtx,
	snip: Snippet<'a, A, X>,
	tok: Tok<String>,
) -> ParseRes<'a, (), A, X> {
	let Parsed { output: head, tail } = try_pop_no_fluff(ctx, snip).await?;
	match &head.tok {
		Token::Name(n) if *n == tok => Ok(Parsed { output: (), tail }),
		t => Err(mk_errv(
			ctx.i().i("Expected specific keyword").await,
			format!("Expected {tok} but found {:?}", fmt(t, ctx.i()).await),
			[Pos::Range(head.range.clone()).into()],
		)),
	}
}

pub struct Parsed<'a, T, H: ExprRepr, X: ExtraTok> {
	pub output: T,
	pub tail: Snippet<'a, H, X>,
}

pub type ParseRes<'a, T, H, X> = OrcRes<Parsed<'a, T, H, X>>;

pub async fn parse_multiname<'a, A: ExprRepr, X: ExtraTok>(
	ctx: &impl ParseCtx,
	tail: Snippet<'a, A, X>,
) -> ParseRes<'a, Vec<(Import, Pos)>, A, X> {
	let Some((tt, tail)) = tail.skip_fluff().pop_front() else {
		return Err(mk_errv(
			ctx.i().i("Expected token").await,
			"Expected a name, a parenthesized list of names, or a globstar.",
			[Pos::Range(tail.pos()).into()],
		));
	};
	let ret = rec(tt, ctx).await;
	#[allow(clippy::type_complexity)] // it's an internal function
	pub async fn rec<A: ExprRepr, X: ExtraTok>(
		tt: &TokTree<A, X>,
		ctx: &impl ParseCtx,
	) -> OrcRes<Vec<(Vec<Tok<String>>, Option<Tok<String>>, Pos)>> {
		let ttpos = Pos::Range(tt.range.clone());
		match &tt.tok {
			Token::NS(ns, body) => {
				if !ns.starts_with(name_start) {
					ctx.reporter().report(mk_err(
						ctx.i().i("Unexpected name prefix").await,
						"Only names can precede ::",
						[ttpos.into()],
					))
				};
				let out = Box::pin(rec(body, ctx)).await?;
				Ok(out.into_iter().update(|i| i.0.push(ns.clone())).collect_vec())
			},
			Token::Name(ntok) => {
				let n = ntok;
				let nopt = Some(n.clone());
				Ok(vec![(vec![], nopt, Pos::Range(tt.range.clone()))])
			},
			Token::S(Paren::Round, b) => {
				let mut o = Vec::new();
				let mut body = Snippet::new(tt, b);
				while let Some((output, tail)) = body.pop_front() {
					match rec(output, ctx).boxed_local().await {
						Ok(names) => o.extend(names),
						Err(e) => ctx.reporter().report(e),
					}
					body = tail;
				}
				Ok(o)
			},
			t => {
				return Err(mk_errv(
					ctx.i().i("Unrecognized name end").await,
					format!("Names cannot end with {:?} tokens", fmt(t, ctx.i()).await),
					[ttpos.into()],
				));
			},
		}
	}
	ret.map(|output| {
		let output = (output.into_iter())
			.map(|(p, name, pos)| (Import { path: VPath::new(p.into_iter().rev()), name }, pos))
			.collect_vec();
		Parsed { output, tail }
	})
}

/// A compound name, possibly ending with a globstar
#[derive(Debug, Clone)]
pub struct Import {
	pub path: VPath,
	pub name: Option<Tok<String>>,
}

impl Display for Import {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}::{}", self.path.iter().join("::"), self.name.as_ref().map_or("*", |t| t.as_str()))
	}
}

#[cfg(test)]
mod test {
	use never::Never;

	use super::Snippet;

	fn _covary_snip_a<'a>(x: Snippet<'static, Never, Never>) -> Snippet<'a, Never, Never> { x }
}
