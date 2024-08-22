use std::borrow::Borrow;
use std::cell::RefCell;
use std::fmt::{self, Display, Write};
use std::iter;
use std::marker::PhantomData;
use std::ops::Range;
use std::sync::Arc;

use itertools::Itertools;
use never::Never;
use trait_set::trait_set;

use crate::api;
use crate::error::OrcErr;
use crate::interner::{deintern, Tok};
use crate::name::{NameLike, VName};
use crate::tokens::PARENS;

trait_set! {
  pub trait RecurCB<'a, A: AtomInTok, X> = Fn(TokTree<'a, A, X>) -> TokTree<'a, A, X>;
}

pub fn recur<'a, A: AtomInTok, X>(
  tt: TokTree<'a, A, X>,
  f: &impl Fn(TokTree<'a, A, X>, &dyn RecurCB<'a, A, X>) -> TokTree<'a, A, X>,
) -> TokTree<'a, A, X> {
  f(tt, &|TokTree { range, tok }| {
    let tok = match tok {
      tok @ (Token::Atom(_) | Token::BR | Token::Bottom(_) | Token::Comment(_) | Token::NS) => tok,
      tok @ (Token::Name(_) | Token::Slot(_) | Token::X(_)) => tok,
      Token::LambdaHead(arg) =>
        Token::LambdaHead(arg.into_iter().map(|tt| recur(tt, f)).collect_vec()),
      Token::S(p, b) => Token::S(p, b.into_iter().map(|tt| recur(tt, f)).collect_vec()),
    };
    TokTree { range, tok }
  })
}

pub trait AtomInTok: Display + Clone {
  type Context: ?Sized;
  fn from_api(atom: &api::Atom, pos: Range<u32>, ctx: &mut Self::Context) -> Self;
  fn to_api(&self) -> api::Atom;
}
impl AtomInTok for Never {
  type Context = Never;
  fn from_api(_: &api::Atom, _: Range<u32>, _: &mut Self::Context) -> Self { panic!() }
  fn to_api(&self) -> orchid_api::Atom { match *self {} }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TreeHandle<'a>(api::TreeTicket, PhantomData<&'a ()>);
impl TreeHandle<'static> {
  pub fn new(tt: api::TreeTicket) -> Self { TreeHandle(tt, PhantomData) }
}
impl<'a> TreeHandle<'a> {
  pub fn ticket(self) -> api::TreeTicket { self.0 }
}
impl<'a> Display for TreeHandle<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Handle({})", self.0.0) }
}

#[derive(Clone, Debug)]
pub struct TokTree<'a, A: AtomInTok, X> {
  pub tok: Token<'a, A, X>,
  pub range: Range<u32>,
}
impl<'a, A: AtomInTok, X> TokTree<'a, A, X> {
  pub fn from_api(tt: &api::TokenTree, ctx: &mut A::Context) -> Self {
    let tok = match &tt.token {
      api::Token::Atom(a) => Token::Atom(A::from_api(a, tt.range.clone(), ctx)),
      api::Token::BR => Token::BR,
      api::Token::NS => Token::NS,
      api::Token::Bottom(e) => Token::Bottom(e.iter().map(OrcErr::from_api).collect()),
      api::Token::Lambda(arg) => Token::LambdaHead(ttv_from_api(arg, ctx)),
      api::Token::Name(name) => Token::Name(deintern(*name)),
      api::Token::S(par, b) => Token::S(par.clone(), ttv_from_api(b, ctx)),
      api::Token::Comment(c) => Token::Comment(c.clone()),
      api::Token::Slot(id) => Token::Slot(TreeHandle::new(*id)),
    };
    Self { range: tt.range.clone(), tok }
  }

  pub fn to_api(
    &self,
    do_extra: &mut impl FnMut(&X, Range<u32>) -> api::TokenTree,
  ) -> api::TokenTree {
    let token = match &self.tok {
      Token::Atom(a) => api::Token::Atom(a.to_api()),
      Token::BR => api::Token::BR,
      Token::NS => api::Token::NS,
      Token::Bottom(e) => api::Token::Bottom(e.iter().map(OrcErr::to_api).collect()),
      Token::Comment(c) => api::Token::Comment(c.clone()),
      Token::LambdaHead(arg) =>
        api::Token::Lambda(arg.iter().map(|t| t.to_api(do_extra)).collect_vec()),
      Token::Name(n) => api::Token::Name(n.marker()),
      Token::Slot(tt) => api::Token::Slot(tt.ticket()),
      Token::S(p, b) => api::Token::S(p.clone(), b.iter().map(|t| t.to_api(do_extra)).collect()),
      Token::X(x) => return do_extra(x, self.range.clone()),
    };
    api::TokenTree { range: self.range.clone(), token }
  }
}
impl<'a, A: AtomInTok + Display, X: Display> Display for TokTree<'a, A, X> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.tok) }
}

pub fn ttv_from_api<A: AtomInTok, X>(
  tokv: impl IntoIterator<Item: Borrow<api::TokenTree>>,
  ctx: &mut A::Context,
) -> Vec<TokTree<'static, A, X>> {
  tokv.into_iter().map(|t| TokTree::<A, X>::from_api(t.borrow(), ctx)).collect()
}

pub fn ttv_to_api<'a, A: AtomInTok, X>(
  tokv: impl IntoIterator<Item: Borrow<TokTree<'a, A, X>>>,
  do_extra: &mut impl FnMut(&X, Range<u32>) -> api::TokenTree,
) -> Vec<api::TokenTree> {
  tokv
    .into_iter()
    .map(|tok| {
      let tt: &TokTree<A, X> = tok.borrow();
      tt.to_api(do_extra)
    })
    .collect_vec()
}

pub fn vname_tv<'a: 'b, 'b, A: AtomInTok + 'a, X: 'a>(
  name: &'b VName,
  ran: Range<u32>,
) -> impl Iterator<Item = TokTree<'a, A, X>> + 'b {
  let (head, tail) = name.split_first();
  iter::once(Token::Name(head))
    .chain(tail.iter().flat_map(|t| [Token::NS, Token::Name(t)]))
    .map(move |t| t.at(ran.clone()))
}

pub fn wrap_tokv<'a, A: AtomInTok + 'a, X: 'a>(
  items: Vec<TokTree<'a, A, X>>,
  range: Range<u32>,
) -> TokTree<'a, A, X> {
  match items.len() {
    1 => items.into_iter().next().unwrap(),
    _ => Token::S(api::Paren::Round, items).at(range),
  }
}

pub use api::Paren;

#[derive(Clone, Debug)]
pub enum Token<'a, A: AtomInTok, X> {
  Comment(Arc<String>),
  LambdaHead(Vec<TokTree<'a, A, X>>),
  Name(Tok<String>),
  NS,
  BR,
  S(Paren, Vec<TokTree<'a, A, X>>),
  Atom(A),
  Bottom(Vec<OrcErr>),
  Slot(TreeHandle<'a>),
  X(X),
}
impl<'a, A: AtomInTok, X> Token<'a, A, X> {
  pub fn at(self, range: Range<u32>) -> TokTree<'a, A, X> { TokTree { range, tok: self } }
}
impl<'a, A: AtomInTok + Display, X: Display> Display for Token<'a, A, X> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    thread_local! {
      static PAREN_LEVEL: RefCell<usize> = 0.into();
    }
    fn get_indent() -> usize { PAREN_LEVEL.with_borrow(|t| *t) }
    fn with_indent<T>(f: impl FnOnce() -> T) -> T {
      PAREN_LEVEL.with_borrow_mut(|t| *t += 1);
      let r = f();
      PAREN_LEVEL.with_borrow_mut(|t| *t -= 1);
      r
    }
    match self {
      Self::Atom(a) => f.write_str(&indent(&format!("{a}"), get_indent(), false)),
      Self::BR => write!(f, "\n{}", "  ".repeat(get_indent())),
      Self::Bottom(err) => write!(
        f,
        "Botttom({})",
        err.iter().map(|e| format!("{}: {}", e.description, e.message)).join(", ")
      ),
      Self::Comment(c) => write!(f, "--[{c}]--"),
      Self::LambdaHead(arg) => with_indent(|| write!(f, "\\ {} .", ttv_fmt(arg))),
      Self::NS => f.write_str("::"),
      Self::Name(n) => f.write_str(n),
      Self::Slot(th) => write!(f, "{th}"),
      Self::S(p, b) => {
        let (lp, rp, _) = PARENS.iter().find(|(_, _, par)| par == p).unwrap();
        f.write_char(*lp)?;
        with_indent(|| f.write_str(&ttv_fmt(b)))?;
        f.write_char(*rp)
      },
      Self::X(x) => write!(f, "{x}"),
    }
  }
}

pub fn ttv_fmt<'a>(
  ttv: impl IntoIterator<Item = &'a TokTree<'a, impl AtomInTok + 'a, impl Display + 'a>>,
) -> String {
  ttv.into_iter().join(" ")
}

pub fn indent(s: &str, lvl: usize, first: bool) -> String {
  if first {
    s.replace("\n", &("\n".to_string() + &"  ".repeat(lvl)))
  } else if let Some((fst, rest)) = s.split_once('\n') {
    fst.to_string() + "\n" + &indent(rest, lvl, true)
  } else {
    s.to_string()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_covariance() {
    fn _f<'a>(x: Token<'static, Never, ()>) -> Token<'a, Never, ()> { x }
  }

  #[test]
  fn fail_covariance() {
    // this fails to compile
    // fn _f<'a, 'b>(x: &'a mut &'static ()) -> &'a mut &'b () { x }
    // this passes because it's covariant
    fn _f<'a, 'b>(x: &'a fn() -> &'static ()) -> &'a fn() -> &'b () { x }
  }
}
