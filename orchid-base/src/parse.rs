use std::iter;
use std::ops::{Deref, Range};

use futures::future::join_all;
use itertools::Itertools;

use crate::api;
use crate::error::{OrcRes, Reporter, mk_err, mk_errv};
use crate::interner::{Internable, Interned, Interner, Tok};
use crate::location::Pos;
use crate::name::VPath;
use crate::tree::{AtomRepr, ExtraTok, Paren, TokTree, Token};

pub fn name_start(c: char) -> bool { c.is_alphabetic() || c == '_' }
pub fn name_char(c: char) -> bool { name_start(c) || c.is_numeric() }
pub fn op_char(c: char) -> bool { !name_char(c) && !c.is_whitespace() && !"()[]{}\\".contains(c) }
pub fn unrep_space(c: char) -> bool { c.is_whitespace() && !"\r\n".contains(c) }

#[derive(Debug)]
pub struct Snippet<'a, 'b, A: AtomRepr, X: ExtraTok> {
	prev: &'a TokTree<'b, A, X>,
	cur: &'a [TokTree<'b, A, X>],
	interner: &'a Interner,
}
impl<'a, 'b, A: AtomRepr, X: ExtraTok> Snippet<'a, 'b, A, X> {
	pub fn new(
		prev: &'a TokTree<'b, A, X>,
		cur: &'a [TokTree<'b, A, X>],
		interner: &'a Interner,
	) -> Self {
		Self { prev, cur, interner }
	}
	pub async fn i<T: Interned>(&self, arg: &(impl Internable<Interned = T> + ?Sized)) -> Tok<T> {
		self.interner.i(arg).await
	}
	pub fn interner(&self) -> &'a Interner { self.interner }
	pub fn split_at(self, pos: u32) -> (Self, Self) {
		let Self { prev, cur, interner } = self;
		let fst = Self { prev, cur: &cur[..pos as usize], interner };
		let new_prev = if pos == 0 { self.prev } else { &self.cur[pos as usize - 1] };
		let snd = Self { prev: new_prev, cur: &self.cur[pos as usize..], interner };
		(fst, snd)
	}
	pub fn find_idx(self, mut f: impl FnMut(&Token<'b, A, X>) -> bool) -> Option<u32> {
		self.cur.iter().position(|t| f(&t.tok)).map(|t| t as u32)
	}
	pub fn get(self, idx: u32) -> Option<&'a TokTree<'b, A, X>> { self.cur.get(idx as usize) }
	pub fn len(self) -> u32 { self.cur.len() as u32 }
	pub fn prev(self) -> &'a TokTree<'b, A, X> { self.prev }
	pub fn pos(self) -> Range<u32> {
		(self.cur.first().map(|f| f.range.start..self.cur.last().unwrap().range.end))
			.unwrap_or(self.prev.range.clone())
	}
	pub fn pop_front(self) -> Option<(&'a TokTree<'b, A, X>, Self)> {
		self.cur.first().map(|r| (r, self.split_at(1).1))
	}
	pub fn pop_back(self) -> Option<(Self, &'a TokTree<'b, A, X>)> {
		self.cur.last().map(|r| (self.split_at(self.len() - 1).0, r))
	}
	pub fn split_once(self, f: impl FnMut(&Token<'b, A, X>) -> bool) -> Option<(Self, Self)> {
		let idx = self.find_idx(f)?;
		Some((self.split_at(idx).0, self.split_at(idx + 1).1))
	}
	pub fn split(
		mut self,
		mut f: impl FnMut(&Token<'b, A, X>) -> bool,
	) -> impl Iterator<Item = Self> {
		iter::from_fn(move || {
			self.is_empty().then_some(())?;
			let (ret, next) = self.split_once(&mut f).unwrap_or(self.split_at(self.len()));
			self = next;
			Some(ret)
		})
	}
	pub fn is_empty(self) -> bool { self.len() == 0 }
	pub fn skip_fluff(self) -> Self {
		let non_fluff_start = self.find_idx(|t| !matches!(t, Token::NS | Token::Comment(_)));
		self.split_at(non_fluff_start.unwrap_or(self.len())).1
	}
}
impl<A: AtomRepr, X: ExtraTok> Copy for Snippet<'_, '_, A, X> {}
impl<A: AtomRepr, X: ExtraTok> Clone for Snippet<'_, '_, A, X> {
	fn clone(&self) -> Self { *self }
}
impl<'b, A: AtomRepr, X: ExtraTok> Deref for Snippet<'_, 'b, A, X> {
	type Target = [TokTree<'b, A, X>];
	fn deref(&self) -> &Self::Target { self.cur }
}

/// Remove tokens that aren't meaningful in expression context, such as comments
/// or line breaks
pub fn strip_fluff<'a, A: AtomRepr, X: ExtraTok>(
	tt: &TokTree<'a, A, X>,
) -> Option<TokTree<'a, A, X>> {
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
	pub pos: Pos,
}
impl Comment {
	pub fn to_api(&self) -> api::Comment {
		api::Comment { location: self.pos.to_api(), text: self.text.to_api() }
	}
	pub async fn from_api(api: &api::Comment, i: &Interner) -> Self {
		Self { pos: Pos::from_api(&api.location, i).await, text: Tok::from_api(api.text, i).await }
	}
}

pub async fn line_items<'a, 'b, A: AtomRepr, X: ExtraTok>(
	snip: Snippet<'a, 'b, A, X>,
) -> Vec<Parsed<'a, 'b, Vec<Comment>, A, X>> {
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
					match &t.tok {
						Token::Comment(c) =>
							Comment { text: tail.i(&**c).await, pos: Pos::Range(t.range.clone()) },
						_ => unreachable!("All are comments checked above"),
					}
				}))
				.await;
				items.push(Parsed { output: comments, tail });
			},
		}
	}
	items
}

pub async fn try_pop_no_fluff<'a, 'b, A: AtomRepr, X: ExtraTok>(
	snip: Snippet<'a, 'b, A, X>,
) -> ParseRes<'a, 'b, &'a TokTree<'b, A, X>, A, X> {
	match snip.skip_fluff().pop_front() {
		Some((output, tail)) => Ok(Parsed { output, tail }),
		None => Err(mk_errv(snip.i("Unexpected end").await, "Pattern ends abruptly", [Pos::Range(
			snip.pos(),
		)
		.into()])),
	}
}

pub async fn expect_end(snip: Snippet<'_, '_, impl AtomRepr, impl ExtraTok>) -> OrcRes<()> {
	match snip.skip_fluff().get(0) {
		Some(surplus) => Err(mk_errv(
			snip.i("Extra code after end of line").await,
			"Code found after the end of the line",
			[Pos::Range(surplus.range.clone()).into()],
		)),
		None => Ok(()),
	}
}

pub async fn expect_tok<'a, 'b, A: AtomRepr, X: ExtraTok>(
	snip: Snippet<'a, 'b, A, X>,
	tok: Tok<String>,
) -> ParseRes<'a, 'b, (), A, X> {
	let Parsed { output: head, tail } = try_pop_no_fluff(snip).await?;
	match &head.tok {
		Token::Name(n) if *n == tok => Ok(Parsed { output: (), tail }),
		t => Err(mk_errv(
			snip.i("Expected specific keyword").await,
			format!("Expected {tok} but found {:?}", t.print().await),
			[Pos::Range(head.range.clone()).into()],
		)),
	}
}

pub struct Parsed<'a, 'b, T, A: AtomRepr, X: ExtraTok> {
	pub output: T,
	pub tail: Snippet<'a, 'b, A, X>,
}

pub type ParseRes<'a, 'b, T, A, X> = OrcRes<Parsed<'a, 'b, T, A, X>>;

pub async fn parse_multiname<'a, 'b, A: AtomRepr, X: ExtraTok>(
	ctx: &(impl Reporter + ?Sized),
	tail: Snippet<'a, 'b, A, X>,
) -> ParseRes<'a, 'b, Vec<(Import, Pos)>, A, X> {
	let ret = rec(ctx, tail).await;
	#[allow(clippy::type_complexity)] // it's an internal function
	pub async fn rec<'a, 'b, A: AtomRepr, X: ExtraTok>(
		ctx: &(impl Reporter + ?Sized),
		tail: Snippet<'a, 'b, A, X>,
	) -> ParseRes<'a, 'b, Vec<(Vec<Tok<String>>, Option<Tok<String>>, Pos)>, A, X> {
		let comma = tail.i(",").await;
		let globstar = tail.i("*").await;
		let Some((name, tail)) = tail.skip_fluff().pop_front() else {
			return Err(mk_errv(
				tail.i("Expected name").await,
				"Expected a name, a list of names, or a globstar.",
				[Pos::Range(tail.pos()).into()],
			));
		};
		if let Some((Token::NS, tail)) = tail.skip_fluff().pop_front().map(|(tt, s)| (&tt.tok, s)) {
			let n = match &name.tok {
				Token::Name(n) if n.starts_with(name_start) => Ok(n),
				_ => Err(mk_err(tail.i("Unexpected name prefix").await, "Only names can precede ::", [
					Pos::Range(name.range.clone()).into(),
				])),
			};
			match (Box::pin(rec(ctx, tail)).await, n) {
				(Err(ev), n) => Err(ev.extended(n.err())),
				(Ok(Parsed { tail, .. }), Err(e)) => {
					ctx.report(e);
					Ok(Parsed { output: vec![], tail })
				},
				(Ok(Parsed { tail, output }), Ok(pre)) => Ok(Parsed {
					output: output.into_iter().update(|i| i.0.push(pre.clone())).collect_vec(),
					tail,
				}),
			}
		} else {
			let output = match &name.tok {
				Token::Name(ntok) => {
					let nopt = match ntok {
						n if *n == globstar => None,
						n if n.starts_with(op_char) => {
							return Err(mk_errv(
								tail.i("Unescaped operator in multiname").await,
								"Operators in multinames should be enclosed in []",
								[Pos::Range(name.range.clone()).into()],
							));
						},
						n => Some(n.clone()),
					};
					vec![(vec![], nopt, Pos::Range(name.range.clone()))]
				},
				Token::S(Paren::Square, b) => {
					let mut ok = Vec::new();
					for tt in b.iter() {
						match &tt.tok {
							Token::Name(n) if n.starts_with(op_char) =>
								ok.push((vec![], Some(n.clone()), Pos::Range(tt.range.clone()))),
							Token::BR | Token::Comment(_) => (),
							_ => ctx.report(mk_err(
								tail.i("Non-operator in escapement in multiname").await,
								"In multinames, [] functions as a literal name list reserved for operators",
								[Pos::Range(name.range.clone()).into()],
							)),
						}
					}
					ok
				},
				Token::S(Paren::Round, b) => {
					let mut ok = Vec::new();
					let body = Snippet::new(name, b, tail.interner);
					for csent in body.split(|n| matches!(n, Token::Name(n) if *n == comma)) {
						match Box::pin(rec(ctx, csent)).await {
							Err(e) => ctx.report(e),
							Ok(Parsed { output, tail }) => match tail.get(0) {
								None => ok.extend(output),
								Some(t) => ctx.report(mk_err(
									tail.i("Unexpected token in multiname group").await,
									"Unexpected token. Likely missing a :: or , or wanted [] instead of ()",
									[Pos::Range(t.range.clone()).into()],
								)),
							},
						}
					}
					ok
				},
				t => {
					return Err(mk_errv(
						tail.i("Unrecognized name end").await,
						format!("Names cannot end with {:?} tokens", t.print().await),
						[Pos::Range(name.range.clone()).into()],
					));
				},
			};
			Ok(Parsed { output, tail })
		}
	}
	ret.map(|Parsed { output, tail }| {
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
impl Import {
	// pub fn from_api(i: api::CompName) -> Self {
	//   Self { path: VPath::new(i.path.into_iter().map(deintern)), name:
	// i.name.map(deintern) } }
	// pub fn to_api(&self) -> api::CompName {
	//   api::CompName {
	//     path: self.path.iter().map(|t| t.marker()).collect(),
	//     name: self.name.as_ref().map(|t| t.marker()),
	//   }
	// }
}

#[cfg(test)]
mod test {
	use never::Never;

	use super::Snippet;

	fn _covary_snip_a<'a, 'b>(
		x: Snippet<'static, 'b, Never, Never>,
	) -> Snippet<'a, 'b, Never, Never> {
		x
	}
	fn _covary_snip_b<'a, 'b>(
		x: Snippet<'a, 'static, Never, Never>,
	) -> Snippet<'a, 'b, Never, Never> {
		x
	}
}
